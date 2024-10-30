use std::{
    fs::File,
    io::{BufReader, Read},
};

use super::constant::{APPIMAGE_MAGIC_BYTES, ELF_MAGIC_BYTES, FLATIMAGE_MAGIC_BYTES};

pub enum FileType {
    AppImage,
    FlatImage,
    ELF,
    Unknown,
}

pub fn get_file_type(file: &mut BufReader<File>) -> FileType {
    let mut magic_bytes = [0u8; 12];
    if file.read_exact(&mut magic_bytes).is_ok() {
        if magic_bytes[8..] == APPIMAGE_MAGIC_BYTES {
            return FileType::AppImage;
        } else if magic_bytes[8..] == FLATIMAGE_MAGIC_BYTES {
            return FileType::FlatImage;
        } else if magic_bytes[..4] == ELF_MAGIC_BYTES {
            return FileType::ELF;
        } else {
            return FileType::Unknown;
        }
    }
    FileType::Unknown
}