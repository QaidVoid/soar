use std::io::{self, Cursor, Read, Write};

use anyhow::Result;
use base64::{engine::general_purpose, Engine};
use icy_sixel::{
    sixel_string, DiffusionMethod, MethodForLargest, MethodForRep, PixelFormat, Quality,
};
use image::{DynamicImage, GenericImageView, ImageFormat, Rgba};
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
    HalfBlock(String),
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

async fn halfblock_string(img: &DynamicImage) -> String {
    let upper_half = '▀';
    let lower_half = '▄';
    let (width, height) = img.dimensions();
    let img_buffer = img.to_rgba8();
    let mut output = String::with_capacity((width * height * 20) as usize);

    let blend_alpha = |pixel: &Rgba<u8>| -> Rgba<u8> {
        if pixel[3] == 255 {
            *pixel
        } else {
            let alpha = pixel[3] as f32 / 255.0;
            Rgba([
                (pixel[0] as f32 * alpha) as u8,
                (pixel[1] as f32 * alpha) as u8,
                (pixel[2] as f32 * alpha) as u8,
                255,
            ])
        }
    };

    let pixel_to_ansi_fg = |pixel: &Rgba<u8>| -> String {
        format!("\x1b[38;2;{};{};{}m", pixel[0], pixel[1], pixel[2])
    };

    let pixel_to_ansi_bg = |pixel: &Rgba<u8>| -> String {
        format!("\x1b[48;2;{};{};{}m", pixel[0], pixel[1], pixel[2])
    };

    let is_transparent = |pixel: &Rgba<u8>| -> bool {
        pixel[3] < 25 // Consider pixels with very low alpha as fully transparent
    };

    for y in (0..height).step_by(2) {
        for x in 0..width {
            let top_pixel = img_buffer.get_pixel(x, y);

            if y + 1 >= height {
                // Last row for odd-height images
                if is_transparent(top_pixel) {
                    output.push(' ');
                } else {
                    output.push_str(&pixel_to_ansi_fg(&blend_alpha(top_pixel)));
                    output.push(upper_half);
                    output.push_str("\x1b[0m");
                }
                continue;
            }

            let bottom_pixel = img_buffer.get_pixel(x, y + 1);
            match (is_transparent(top_pixel), is_transparent(bottom_pixel)) {
                (true, true) => output.push(' '), // Both transparent
                (true, false) => {
                    // Only top pixel visible
                    output.push_str(&pixel_to_ansi_fg(&blend_alpha(bottom_pixel)));
                    output.push(lower_half);
                    output.push_str("\x1b[0m");
                }
                (false, true) => {
                    // Only bottom pixel visible
                    output.push_str(&pixel_to_ansi_fg(&blend_alpha(top_pixel)));
                    output.push(upper_half);
                    output.push_str("\x1b[0m");
                }
                (false, false) => {
                    // Both pixels visible
                    let top = blend_alpha(top_pixel);
                    let bottom = blend_alpha(bottom_pixel);
                    output.push_str(&pixel_to_ansi_fg(&bottom));
                    output.push_str(&pixel_to_ansi_bg(&top));
                    output.push(lower_half);
                    output.push_str("\x1b[0m");
                }
            }
        }
        output.push('\n');
    }

    output
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
                resolved_package.repo_name, resolved_package.collection
            ))
            .await
            .unwrap_or_default(),
        };

        let image_width = (get_font_width() * 30) as u32;
        let image_height = (get_font_height() * 16) as u32;

        let img = match image::load_from_memory(&icon) {
            Ok(img) => img,
            Err(_) => image::load_from_memory(
                &load_default_icon(&format!(
                    "{}-{}.png",
                    resolved_package.repo_name, resolved_package.collection
                ))
                .await
                .unwrap_or_default(),
            )
            .unwrap(),
        };

        if is_kitty_supported().unwrap_or(false) {
            let img = img.resize_exact(
                image_width,
                image_height,
                image::imageops::FilterType::Lanczos3,
            );
            let mut icon = Vec::new();
            let mut cursor = Cursor::new(&mut icon);
            img.write_to(&mut cursor, ImageFormat::Png)
                .expect("Failed to write image in PNG format");

            let encoded = general_purpose::STANDARD.encode(&icon);

            return Self::Kitty(build_transmit_sequence(&encoded));
        } else if is_sixel_supported().unwrap_or(false) {
            let img = img.resize_exact(
                image_width,
                image_height,
                image::imageops::FilterType::Lanczos3,
            );
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

        let img = img.resize_exact(30, 30, image::imageops::FilterType::Lanczos3);
        let halfblock_output = halfblock_string(&img).await;
        Self::HalfBlock(halfblock_output)
    }
}
