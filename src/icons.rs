// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::{Path, PathBuf};

pub fn asset_path(name: &str) -> PathBuf {
    if let Ok(dir) = std::env::var("YOIN_ASSETS") {
        let path = PathBuf::from(&dir).join(name);
        if path.is_file() {
            return path;
        }
    }
    let local = PathBuf::from("assets").join(name);
    if local.is_file() {
        return local;
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            for candidate in [dir.join("assets").join(name), dir.join("../share/yoin").join(name)] {
                if candidate.is_file() {
                    return candidate;
                }
            }
        }
    }
    PathBuf::from("assets").join(name)
}

pub fn load_tray_icon(path: &Path) -> Option<ksni::Icon> {
    let file = std::fs::File::open(path).ok()?;
    let decoder = png::Decoder::new(std::io::BufReader::new(file));
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    let bytes = &buf[..info.buffer_size()];
    let width = info.width as i32;
    let height = info.height as i32;
    if width <= 0 || height <= 0 {
        return None;
    }
    let data = match info.color_type {
        png::ColorType::Rgba => rgba_to_argb(bytes),
        png::ColorType::Rgb => rgb_to_argb(bytes),
        _ => return None,
    };
    Some(ksni::Icon { width, height, data })
}

fn rgba_to_argb(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    for px in bytes.chunks_exact(4) {
        out.extend_from_slice(&[px[3], px[0], px[1], px[2]]);
    }
    out
}

fn rgb_to_argb(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len() / 3 * 4);
    for px in bytes.chunks_exact(3) {
        out.extend_from_slice(&[255, px[0], px[1], px[2]]);
    }
    out
}
