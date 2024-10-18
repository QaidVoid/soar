use std::io::{self, Cursor, Read, Write};

use anyhow::Result;
use base64::{engine::general_purpose, Engine};
use icy_sixel::{
    sixel_string, DiffusionMethod, MethodForLargest, MethodForRep, PixelFormat, Quality,
};
use image::{GenericImageView, ImageFormat};
use termion::raw::IntoRawMode;
use tokio::fs;

use crate::core::{
    constant::REGISTRY_PATH,
    util::{download, get_font_height, get_font_width},
};

use super::ResolvedPackage;

#[derive(Debug, Clone)]
pub enum PackageImage {
    Sixel(String),
    Kitty(String),
    Iterm(String),
    None,
}

fn is_kitty_supported() -> Result<bool> {
    let mut stdout = io::stdout().into_raw_mode()?;
    let sequence = "\x1b_Gi=31,s=1,v=1,a=q,t=d,f=24;AAAA\x1b\\\x1b[c";

    write!(stdout, "{}", sequence)?;
    stdout.flush()?;

    let mut buffer = [0u8; 1024];
    let mut stdin = io::stdin();

    if let Ok(bytes_read) = stdin.read(&mut buffer) {
        if bytes_read > 0 {
            let buf_str = String::from_utf8_lossy(&buffer);
            return Ok(buf_str.contains("OK"));
        }
    }

    Ok(false)
}

fn is_sixel_supported() -> Result<bool> {
    let mut stdout = io::stdout().into_raw_mode()?;
    let sequence = "\x1b[c";

    write!(stdout, "{}", sequence)?;
    stdout.flush()?;

    let mut buffer = Vec::new();
    let stdin = io::stdin();

    for byte in stdin.bytes() {
        let byte = byte?;
        if byte == b'c' {
            break;
        }
        buffer.push(byte);
    }

    let buf_str = String::from_utf8_lossy(&buffer);
    for code in buf_str.split([';', '?']) {
        if code == "4" {
            return Ok(true);
        }
    }
    Ok(false)
}

fn build_transmit_sequence(base64_data: &str) -> String {
    let chunk_size = 4096;
    let mut pos = 0;
    let mut sequence = String::new();

    while pos < base64_data.len() {
        sequence.push_str("\x1b_G");

        if pos == 0 {
            sequence.push_str("a=T,f=100,");
        }

        let end = std::cmp::min(pos + chunk_size, base64_data.len());
        let chunk = &base64_data[pos..end];
        pos = end;

        if pos < base64_data.len() {
            sequence.push_str("m=1");
        }

        if !chunk.is_empty() {
            sequence.push(';');
            sequence.push_str(chunk);
        }
        sequence.push_str("\x1b\\");
    }

    sequence
}

pub async fn load_default_icon(icon_path: &str) -> Result<Vec<u8>> {
    let icon_path = REGISTRY_PATH.join("icons").join(icon_path);
    let content = if icon_path.exists() {
        fs::read(&icon_path).await?
    } else {
        vec![]
    };
    Ok(content)
}

impl PackageImage {
    pub async fn from(resolved_package: &ResolvedPackage) -> Self {
        let package = &resolved_package.package;
        let icon = download(&package.icon, "icon", true).await;
        let icon = match icon {
            Ok(icon) => icon,
            Err(_) => load_default_icon(&format!(
                "{}-{}.png",
                resolved_package.repo_name, resolved_package.root_path
            ))
            .await
            .unwrap_or_default(),
        };

        let image_width = (get_font_width() * 30) as u32;
        let image_height = (get_font_height() * 16) as u32;

        let img = image::load_from_memory(&icon).unwrap();
        let img = img.resize_exact(
            image_width,
            image_height,
            image::imageops::FilterType::Lanczos3,
        );

        if is_kitty_supported().unwrap_or(false) {
            let mut icon = Vec::new();
            let mut cursor = Cursor::new(&mut icon);
            img.write_to(&mut cursor, ImageFormat::Png)
                .expect("Failed to write image in PNG format");

            let encoded = general_purpose::STANDARD.encode(&icon);

            return Self::Kitty(build_transmit_sequence(&encoded));
        } else if is_sixel_supported().unwrap_or(false) {
            let (width, height) = img.dimensions();
            let img_rgba8 = img.to_rgba8();
            let bytes = img_rgba8.as_raw();

            let sixel_output = sixel_string(
                bytes,
                width as i32,
                height as i32,
                PixelFormat::RGBA8888,
                DiffusionMethod::Stucki,
                MethodForLargest::Auto,
                MethodForRep::Auto,
                Quality::HIGH,
            )
            .unwrap();

            return Self::Sixel(sixel_output);
        };
        Self::None
    }
}
