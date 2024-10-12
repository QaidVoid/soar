use std::{
    fs::File,
    io::{BufReader, BufWriter, Read, Seek, Write},
    path::Path,
};

use anyhow::Result;
use backhand::{kind::Kind, FilesystemReader, InnerNode};
use tokio::fs;

use crate::core::util::home_data_path;

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
        if to.read_link().is_ok()
            && xattr::get_deref(from, "user.ManagedBy")?.as_deref() != Some(b"soar")
        {
            eprintln!("{} is not managed by soar", to.to_string_lossy());
            return Ok(());
        }
        fs::remove_file(to).await?;
    }
    fs::symlink(from, to).await?;

    Ok(())
}

async fn remove_link(path: &Path) -> Result<()> {
    if path.exists() {
        if path.read_link().is_ok()
            && xattr::get_deref(path, "user.ManagedBy")?.as_deref() != Some(b"soar")
        {
            eprintln!("{} is not managed by soar", path.to_string_lossy());
            return Ok(());
        }
        fs::remove_file(path).await?;
    }
    Ok(())
}

pub async fn remove_applinks(name: &str) -> Result<()> {
    let home_data = home_data_path();
    let data_path = Path::new(&home_data);
    let icon_path = data_path.join("icons").join(name).with_extension("png");
    let desktop_path = data_path
        .join("applications")
        .join(name)
        .with_extension("desktop");

    remove_link(&icon_path).await?;
    remove_link(&desktop_path).await?;

    Ok(())
}

pub async fn extract_appimage(name: &str, file_path: &Path) -> Result<()> {
    let mut file = BufReader::new(File::open(file_path)?);

    if !is_appimage(&mut file) {
        return Err(anyhow::anyhow!("NOT_APPIMAGE"));
    }

    let offset = find_offset(&mut file).await?;
    let squashfs = FilesystemReader::from_reader_with_offset(file, offset)?;

    let home_data = home_data_path();
    let data_path = Path::new(&home_data);
    let final_icon_path = data_path.join("icons").join(name).with_extension("png");
    let final_desktop_path = data_path
        .join("applications")
        .join(name)
        .with_extension("desktop");

    for node in squashfs.files() {
        let node_path = node.fullpath.to_string_lossy();
        if node_path.ends_with(".png") || node_path.ends_with(".desktop") {
            if let InnerNode::File(file) = &node.inner {
                let mut reader = squashfs.file(&file.basic).reader().bytes();
                let extension = if node_path.ends_with(".png") {
                    "png"
                } else {
                    "desktop"
                };
                let final_path = if extension == "png" {
                    &final_icon_path
                } else {
                    &final_desktop_path
                };
                let output_path = file_path.with_extension(extension);
                let output_file = File::create(&output_path);
                let mut writer = BufWriter::new(output_file?);

                if extension == "png" {
                    while let Some(Ok(byte)) = reader.next() {
                        writer.write_all(&[byte])?;
                    }
                } else {
                    let mut buffer = Vec::new();
                    while let Some(Ok(byte)) = reader.next() {
                        buffer.push(byte);
                    }
                    let content = String::from_utf8(buffer)?;
                    let content = content
                        .lines()
                        .map(|line| {
                            if line.starts_with("Icon=") {
                                format!("Icon={}", final_icon_path.to_string_lossy())
                            } else {
                                line.to_string()
                            }
                        })
                        .collect::<Vec<String>>()
                        .join("\n");

                    writer.write_all(content.as_bytes())?;
                }
                xattr::set(&output_path, "user.ManagedBy", b"soar")?;
                create_symlink(&output_path, final_path).await?;
            }
        }
    }

    Ok(())
}
