use std::io::{self, Cursor, Read, Write};

use anyhow::Result;
use base64::{engine::general_purpose, Engine};
use image::ImageFormat;
use termion::raw::IntoRawMode;

use crate::core::util::{download, get_font_height, get_font_width};

use super::ResolvedPackage;

#[derive(Debug, Clone)]
pub struct PackageImage {
    pub sixel: Option<String>,
    pub kitty: Option<String>,
    pub iterm: Option<String>,
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

impl PackageImage {
    pub async fn from(resolved_package: &ResolvedPackage) -> Self {
        let package = &resolved_package.package;
        let icon = (download(&package.icon, "icon", true).await).unwrap_or_default();

        let font_width = get_font_width();
        let font_height = get_font_height();

        let img = image::load_from_memory(&icon).unwrap();
        let img = img.resize_exact(
            (font_width * 30) as u32,
            (font_height * 16) as u32,
            image::imageops::FilterType::Lanczos3,
        );
        let mut icon = Vec::new();
        let mut cursor = Cursor::new(&mut icon);
        img.write_to(&mut cursor, ImageFormat::Png)
            .expect("Failed to write image in PNG format");

        let encoded = general_purpose::STANDARD.encode(&icon);

        let kitty = if is_kitty_supported().unwrap_or(false) {
            Some(build_transmit_sequence(&encoded))
        } else {
            None
        };

        // Unimplemented
        let sixel = None;
        let iterm = None;

        Self {
            sixel,
            kitty,
            iterm,
        }
    }
}
