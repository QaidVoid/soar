use std::{io::Write, path::Path};

use anyhow::Result;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use tokio::{fs::File, io::AsyncReadExt};

use crate::core::constant::{BIN_PATH, ERROR};

use super::registry::{Package, ResolvedPackage, RootPath};

pub async fn setup_symlink(install_path: &Path, resolved_package: &ResolvedPackage) -> Result<()> {
    let symlink_path = BIN_PATH.join(&resolved_package.package.bin_name);
    if symlink_path.exists() {
        let attr = xattr::get_deref(&symlink_path, "user.owner")?;
        if attr.as_deref() != Some(b"soar") {
            return Err(anyhow::Error::msg(format!(
                "Path {} is not managed by soar. Skipping symlink.",
                symlink_path.to_string_lossy()
            )));
        }
        tokio::fs::remove_file(&symlink_path).await?;
    }
    std::os::unix::fs::symlink(install_path, symlink_path)?;
    Ok(())
}

pub async fn verify_checksum(path: &Path, package: &Package) -> Result<bool> {
    let mut file = File::open(path).await?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0u8; 8192];

    while let Ok(n) = file.read(&mut buffer).await {
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }

    Ok(hasher.finalize().to_hex().to_string() == package.bsum)
}

#[derive(Debug)]
pub struct PackageQuery {
    pub name: String,
    pub variant: Option<String>,
    pub root_path: Option<RootPath>,
}

pub fn parse_package_query(query: &str) -> PackageQuery {
    let (base_query, root_path) = query
        .rsplit_once('#')
        .map(|(n, r)| {
            (
                n.to_owned(),
                match r.to_lowercase().as_str() {
                    "base" => Some(RootPath::Base),
                    "bin" => Some(RootPath::Bin),
                    "pkg" => Some(RootPath::Pkg),
                    _ => {
                        eprintln!("Invalid root path provided for {}", query);
                        std::process::exit(-1);
                    }
                },
            )
        })
        .unwrap_or((query.to_owned(), None));

    let (name, variant) = base_query
        .split_once('/')
        .map(|(v, n)| (n.to_owned(), Some(v.to_owned())))
        .unwrap_or((base_query, None));

    PackageQuery {
        name,
        variant,
        root_path,
    }
}

pub fn select_package_variant(packages: &[ResolvedPackage]) -> Result<&ResolvedPackage> {
    println!(
        "\nMultiple packages available for {}",
        packages[0].package.name
    );
    for (i, package) in packages.iter().enumerate() {
        println!("  {}. {}: {}", i + 1, package, package.package.description);
    }

    let selection = loop {
        print!("Select a variant (1-{}): ", packages.len());
        std::io::stdout().flush()?;

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;

        match input.trim().parse::<usize>() {
            Ok(n) if n > 0 && n <= packages.len() => break n - 1,
            _ => println!("Invalid selection, please try again."),
        }
    };

    Ok(&packages[selection])
}

pub fn set_error(multi_progress: &MultiProgress, msg: &str) {
    let error = multi_progress.insert_from_back(1, ProgressBar::new(0));
    error.set_style(ProgressStyle::with_template("  {msg}").unwrap());
    error.set_message(format!("{} {}", ERROR, msg.to_owned()));
    error.finish();
}
