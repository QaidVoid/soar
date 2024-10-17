use std::{
    cmp::Ordering,
    collections::HashSet,
    fs::File,
    io::{BufReader, BufWriter, Read, Seek, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use backhand::{kind::Kind, FilesystemReader, InnerNode, Node, SquashfsFileReader};
use image::{imageops::FilterType, DynamicImage, GenericImageView};
use libc::{fork, unshare, waitpid, CLONE_NEWUSER};
use tokio::{fs, try_join};

use crate::{
    core::{
        color::{Color, ColorExt},
        constant::{BIN_PATH, PACKAGES_PATH},
        util::{download, home_data_path},
    },
    error, info, warn,
};

use super::Package;

const SUPPORTED_DIMENSIONS: &[(u32, u32)] = &[
    (16, 16),
    (24, 24),
    (32, 32),
    (48, 48),
    (64, 64),
    (72, 72),
    (80, 80),
    (96, 96),
    (128, 128),
    (192, 192),
    (256, 256),
    (512, 512),
];

async fn find_offset(file: &mut BufReader<File>) -> Result<u64> {
    let mut magic = [0_u8; 4];
    // Little-Endian v4.0
    let kind = Kind::from_target("le_v4_0").unwrap();
    while file.read_exact(&mut magic).is_ok() {
        if magic == kind.magic() {
            let found = file.stream_position()? - magic.len() as u64;
            file.rewind()?;
            return Ok(found);
        }
    }
    file.rewind()?;
    Ok(0)
}

fn find_nearest_supported_dimension(width: u32, height: u32) -> (u32, u32) {
    SUPPORTED_DIMENSIONS
        .iter()
        .min_by_key(|&&(w, h)| {
            let width_diff = (w as i32 - width as i32).abs();
            let height_diff = (h as i32 - height as i32).abs();
            width_diff + height_diff
        })
        .cloned()
        .unwrap_or((width, height))
}

fn normalize_image(image: DynamicImage) -> DynamicImage {
    let (width, height) = image.dimensions();
    let (new_width, new_height) = find_nearest_supported_dimension(width, height);

    if (width, height) != (new_width, new_height) {
        info!(
            "Resizing image from {}x{} to {}x{}",
            width, height, new_width, new_height
        );
        image.resize(new_width, new_height, FilterType::Lanczos3)
    } else {
        image
    }
}

fn is_appimage(file: &mut BufReader<File>) -> bool {
    let mut magic_bytes = [0_u8; 16];
    let appimage_bytes = [
        0x7f, 0x45, 0x4c, 0x46, 0x02, 0x01, 0x01, 0x00, 0x41, 0x49, 0x02, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ];
    if file.read_exact(&mut magic_bytes).is_ok() {
        return appimage_bytes == magic_bytes;
    }
    false
}

async fn create_symlink(from: &Path, to: &Path) -> Result<()> {
    if to.exists() {
        if to.read_link().is_ok() && !to.read_link()?.starts_with(&*PACKAGES_PATH) {
            error!(
                "{} is not managed by soar",
                to.to_string_lossy().color(Color::Blue)
            );
            return Ok(());
        }
        fs::remove_file(to).await?;
    }
    fs::symlink(from, to).await?;

    Ok(())
}

async fn remove_link(path: &Path) -> Result<()> {
    if path.exists() {
        if path.read_link().is_ok() && !path.read_link()?.starts_with(&*PACKAGES_PATH) {
            error!(
                "{} is not managed by soar",
                path.to_string_lossy().color(Color::Blue)
            );
            return Ok(());
        }
        fs::remove_file(path).await?;
    }
    Ok(())
}

pub async fn remove_applinks(name: &str, bin_name: &str, file_path: &Path) -> Result<()> {
    let home_data = home_data_path();
    let data_path = Path::new(&home_data);

    let original_icon_path = file_path.with_extension("png");
    let (w, h) = image::image_dimensions(&original_icon_path)?;
    let icon_path = data_path
        .join("icons")
        .join("hicolor")
        .join(format!("{}x{}", w, h))
        .join("apps")
        .join(bin_name)
        .with_extension("png");
    let desktop_path = data_path
        .join("applications")
        .join(format!("{name}-soar.desktop"));

    remove_link(&desktop_path).await?;
    remove_link(&icon_path).await?;

    Ok(())
}

pub async fn extract_appimage(package: &Package, file_path: &Path) -> Result<()> {
    let mut file = BufReader::new(File::open(file_path)?);

    if !is_appimage(&mut file) {
        use_remote_files(package, file_path).await?;
        return Ok(());
    }

    let offset = find_offset(&mut file).await?;
    let squashfs = FilesystemReader::from_reader_with_offset(file, offset)?;

    let home_data = home_data_path();
    let data_path = Path::new(&home_data);

    for node in squashfs.files() {
        let node_path = node.fullpath.to_string_lossy();
        if !node_path.trim_start_matches("/").contains("/")
            && (node_path.ends_with(".DirIcon") || node_path.ends_with(".desktop"))
        {
            let extension = if node_path.ends_with(".DirIcon") {
                "png"
            } else {
                "desktop"
            };
            let output_path = file_path.with_extension(extension);
            match resolve_and_extract(&squashfs, node, &output_path, &mut HashSet::new()) {
                Ok(()) => {
                    if extension == "png" {
                        process_icon(&output_path, &package.bin_name, data_path).await?;
                    } else {
                        process_desktop(&output_path, &package.bin_name, &package.name, data_path)
                            .await?;
                    }
                }
                Err(e) => error!("Failed to extract {}: {}", node_path.color(Color::Blue), e),
            }
        }
    }

    Ok(())
}

fn resolve_and_extract(
    squashfs: &FilesystemReader,
    node: &Node<SquashfsFileReader>,
    output_path: &Path,
    visited: &mut HashSet<PathBuf>,
) -> Result<()> {
    match &node.inner {
        InnerNode::File(file) => extract_file(squashfs, file, output_path),
        InnerNode::Symlink(sym) => {
            let target_path = sym.link.clone();
            if !visited.insert(target_path.clone()) {
                return Err(anyhow::anyhow!(
                    "Uh oh. Bad symlink.. Infinite recursion detected..."
                ));
            }
            if let Some(target_node) = squashfs
                .files()
                .find(|n| n.fullpath.strip_prefix("/").unwrap() == target_path)
            {
                resolve_and_extract(squashfs, target_node, output_path, visited)
            } else {
                Err(anyhow::anyhow!("Symlink target not found"))
            }
        }
        _ => Err(anyhow::anyhow!("Unexpected node type")),
    }
}

fn extract_file(
    squashfs: &FilesystemReader,
    file: &SquashfsFileReader,
    output_path: &Path,
) -> Result<()> {
    let mut reader = squashfs.file(&file.basic).reader().bytes();
    let output_file = File::create(output_path)?;
    let mut buf_writer = BufWriter::new(output_file);
    while let Some(Ok(byte)) = reader.next() {
        buf_writer.write_all(&[byte])?;
    }
    Ok(())
}

async fn process_icon(output_path: &Path, name: &str, data_path: &Path) -> Result<()> {
    let image = image::open(output_path)?;
    let (orig_w, orig_h) = image.dimensions();

    let normalized_image = normalize_image(image);
    let (w, h) = normalized_image.dimensions();

    if (w, h) != (orig_w, orig_h) {
        normalized_image.save(output_path)?;
    }
    let final_path = data_path
        .join("icons")
        .join("hicolor")
        .join(format!("{}x{}", w, h))
        .join("apps")
        .join(name)
        .with_extension("png");

    if let Some(parent) = final_path.parent() {
        fs::create_dir_all(parent).await.context(anyhow::anyhow!(
            "Failed to create icon directory at {}",
            parent.to_string_lossy().color(Color::Blue)
        ))?;
    }
    create_symlink(output_path, &final_path).await?;
    Ok(())
}

async fn process_desktop(
    output_path: &Path,
    bin_name: &str,
    name: &str,
    data_path: &Path,
) -> Result<()> {
    let mut content = String::new();
    File::open(output_path)?.read_to_string(&mut content)?;

    let processed_content = content
        .lines()
        .filter(|line| !line.starts_with('#'))
        .map(|line| {
            if line.starts_with("Icon=") {
                format!("Icon={}", bin_name)
            } else if line.starts_with("Exec=") {
                format!("Exec={}/{}", &*BIN_PATH.to_string_lossy(), bin_name)
            } else if line.starts_with("TryExec=") {
                format!("TryExec={}/{}", &*BIN_PATH.to_string_lossy(), bin_name)
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<String>>()
        .join("\n");

    let mut writer = BufWriter::new(File::create(output_path)?);
    writer.write_all(processed_content.as_bytes())?;

    let final_path = data_path
        .join("applications")
        .join(format!("{name}-soar.desktop"));

    if let Some(parent) = final_path.parent() {
        fs::create_dir_all(parent).await.context(anyhow::anyhow!(
            "Failed to create desktop files directory at {}",
            parent.to_string_lossy().color(Color::Blue)
        ))?;
    }

    create_symlink(output_path, &final_path).await?;
    Ok(())
}

pub async fn use_remote_files(package: &Package, file_path: &Path) -> Result<()> {
    let home_data = home_data_path();
    let data_path = Path::new(&home_data);

    let icon_output_path = file_path.with_extension("png");
    let desktop_output_path = file_path.with_extension("desktop");

    let icon_url = &package.icon;
    let (base_url, _) = package.icon.rsplit_once('/').unwrap();
    let desktop_url = format!("{}/{}.desktop", base_url, &package.bin_name);

    let (icon_content, desktop_content) = try_join!(
        download(icon_url, "image", false),
        download(&desktop_url, "desktop file", false)
    )?;

    try_join!(
        fs::write(&icon_output_path, &icon_content),
        fs::write(&desktop_output_path, &desktop_content)
    )?;

    try_join!(
        process_icon(&icon_output_path, &package.bin_name, data_path),
        process_desktop(
            &desktop_output_path,
            &package.bin_name,
            &package.name,
            data_path
        )
    )?;

    Ok(())
}

pub async fn setup_portable_dir(
    bin_name: &str,
    package_path: &Path,
    portable: Option<String>,
    portable_home: Option<String>,
    portable_config: Option<String>,
) -> Result<()> {
    let pkg_config = package_path.with_extension("config");
    let pkg_home = package_path.with_extension("home");

    let (portable_home, portable_config) = if let Some(portable) = portable {
        (Some(portable.clone()), Some(portable.clone()))
    } else {
        (portable_home, portable_config)
    };

    if let Some(portable_home) = portable_home {
        if portable_home.is_empty() {
            fs::create_dir(&pkg_home).await?;
        } else {
            let portable_home = PathBuf::from(portable_home)
                .join(bin_name)
                .with_extension("home");
            fs::create_dir_all(&portable_home)
                .await
                .context(anyhow::anyhow!(
                    "Failed to create or access directory at {}",
                    &portable_home.to_string_lossy().color(Color::Blue)
                ))?;
            create_symlink(&portable_home, &pkg_home).await?;
        }
    }
    if let Some(portable_config) = portable_config {
        if portable_config.is_empty() {
            fs::create_dir(&pkg_config).await?;
        } else {
            let portable_config = PathBuf::from(portable_config)
                .join(bin_name)
                .with_extension("config");
            fs::create_dir_all(&portable_config)
                .await
                .context(anyhow::anyhow!(
                    "Failed to create or access directory at {}",
                    &portable_config.to_string_lossy().color(Color::Blue)
                ))?;
            create_symlink(&portable_config, &pkg_config).await?;
        }
    }

    Ok(())
}

pub async fn check_user_ns() {
    let mut errors = Vec::new();

    let pid = unsafe { fork() };
    match pid.cmp(&0) {
        Ordering::Equal => {
            if unsafe { unshare(CLONE_NEWUSER) != 0 } {
                errors.push("You lack permissions to create user_namespaces");
            }
            std::process::exit(0);
        }
        Ordering::Greater => {
            unsafe {
                waitpid(pid, std::ptr::null_mut(), 0);
            };
        }
        _ => {}
    }

    if !Path::new("/proc/self/ns/user").exists() {
        errors.push("Your kernel does not support user namespaces");
    }
    if let Ok(content) = fs::read_to_string("/proc/sys/kernel/unprivileged_userns_clone").await {
        if content.trim() == "0" {
            errors.push("You must enable unprivileged_userns_clone");
        }
    }
    if let Ok(content) = fs::read_to_string("/proc/sys/user/max_user_namespaces").await {
        if content.trim() == "0" {
            errors.push("You must enable max_user_namespaces");
        }
    }
    if let Ok(content) = fs::read_to_string("/proc/sys/kernel/userns_restrict").await {
        if content.trim() == "1" {
            errors.push("You must disable userns_restrict");
        }
    }
    if let Ok(content) =
        fs::read_to_string("/proc/sys/kernel/apparmor_restrict_unprivileged_userns").await
    {
        if content.trim() == "1" {
            errors.push("You must disable apparmor_restrict_unprivileged_userns");
        }
    }
    if !errors.is_empty() {
        for error in errors {
            warn!("{}", error);
            println!(
                "{} {}",
                "More info at:".color(Color::Cyan),
                "https://l.ajam.dev/namespace".color(Color::Blue)
            )
        }
    }
}
