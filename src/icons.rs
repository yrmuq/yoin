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

pub struct ThemeIcon {
    pub image: slint::Image,
    pub tint: bool,
}

pub const BUTTON_ICONS: &[(&str, u32)] = &[
    ("", 80), ("light", 81), ("switch", 82), ("lock", 83),
    ("alarm_control_panel", 84), ("climate", 85), ("cover", 86), ("fan", 87),
    ("media_player", 88), ("script", 89), ("automation", 90), ("siren", 91),
    ("remote", 92), ("valve", 93), ("group", 94), ("camera", 95), ("door", 96),
];

fn bundled(name: &str) -> Option<&'static [u8]> {
    Some(match name {
        "flashlight-off" => include_bytes!("../ui/icons/flashlight-off.svg"),
        "flashlight-on" => include_bytes!("../ui/icons/flashlight-on.svg"),
        "reverse" => include_bytes!("../ui/icons/reverse.svg"),
        "object-locked" => include_bytes!("../ui/icons/object-locked.svg"),
        "object-unlocked" => include_bytes!("../ui/icons/object-unlocked.svg"),
        "security-low" => include_bytes!("../ui/icons/security-low.svg"),
        "security-high" => include_bytes!("../ui/icons/security-high.svg"),
        "temperature-cold" => include_bytes!("../ui/icons/temperature-cold.svg"),
        "temperature-warm" => include_bytes!("../ui/icons/temperature-warm.svg"),
        "window-shade" => include_bytes!("../ui/icons/window-shade.svg"),
        "view-refresh" => include_bytes!("../ui/icons/view-refresh.svg"),
        "media-playback-start" => include_bytes!("../ui/icons/media-playback-start.svg"),
        "media-playback-stop" => include_bytes!("../ui/icons/media-playback-stop.svg"),
        "document-edit" => include_bytes!("../ui/icons/document-edit.svg"),
        "system-run" => include_bytes!("../ui/icons/system-run.svg"),
        "audio-volume-muted" => include_bytes!("../ui/icons/audio-volume-muted.svg"),
        "audio-volume-high" => include_bytes!("../ui/icons/audio-volume-high.svg"),
        "input-keyboard" => include_bytes!("../ui/icons/input-keyboard.svg"),
        "network-wired" => include_bytes!("../ui/icons/network-wired.svg"),
        "network-connect" => include_bytes!("../ui/icons/network-connect.svg"),
        "view-group" => include_bytes!("../ui/icons/view-group.svg"),
        "camera-photo" => include_bytes!("../ui/icons/camera-photo.svg"),
        "window" => include_bytes!("../ui/icons/window.svg"),
        "dialog-cancel" => include_bytes!("../ui/icons/dialog-cancel.svg"),
        "dialog-ok-apply" => include_bytes!("../ui/icons/dialog-ok-apply.svg"),
        _ => return None,
    })
}

pub fn button_icon(kind: &str, on: bool, edge: u32) -> Option<ThemeIcon> {
    for name in icon_candidates(kind, on) {
        let Some(bytes) = bundled(name) else { continue };
        let image = if *name == "reverse" {
            turn_on_side(svg_image(bytes, edge)?)
        } else {
            slint::Image::load_from_svg_data(bytes).ok()?
        };
        return Some(ThemeIcon { image, tint: true });
    }
    None
}

fn icon_candidates(kind: &str, on: bool) -> &'static [&'static str] {
    match (kind, on) {
        ("light", false) => &["flashlight-off", "brightness-low"],
        ("light", true) => &["flashlight-on", "brightness-high"],
        ("switch" | "input_boolean", _) => &["reverse"],
        ("lock", false) => &["object-unlocked", "unlock"],
        ("lock", true) => &["object-locked", "system-lock-screen"],
        ("alarm_control_panel", false) => &["security-low"],
        ("alarm_control_panel", true) => &["security-high"],
        ("climate" | "water_heater" | "humidifier", false) => &["temperature-cold"],
        ("climate" | "water_heater" | "humidifier", true) => &["temperature-warm"],
        ("door", _) => &["window"],
        ("cover", _) => &["window-shade"],
        ("fan", _) => &["view-refresh"],
        ("media_player", false) => &["media-playback-stop"],
        ("media_player", true) => &["media-playback-start"],
        ("script", _) => &["document-edit"],
        ("automation", _) => &["system-run"],
        ("siren", false) => &["audio-volume-muted"],
        ("siren", true) => &["audio-volume-high"],
        ("remote", _) => &["input-keyboard"],
        ("camera", false) => &["camera-off", "camera-photo"],
        ("camera", true) => &["camera-on", "camera-photo"],
        ("valve", false) => &["network-wired"],
        ("valve", true) => &["network-connect"],
        ("group", _) => &["view-group"],
        (_, false) => &["dialog-cancel"],
        (_, true) => &["dialog-ok-apply"],
    }
}

pub fn theme_icon(name: &str) -> Option<ThemeIcon> {
    let path = locate_icon(name)?;
    let tint = should_tint(&path);
    let image = load_slint_image(&path)?;
    Some(ThemeIcon { image, tint })
}

fn locate_icon(name: &str) -> Option<PathBuf> {
    let mut seen = std::collections::HashSet::new();
    let theme = icon_theme_name();
    find_icon(&theme, name, &mut seen)
        .or_else(|| find_icon("breeze", name, &mut seen))
        .or_else(|| find_icon("hicolor", name, &mut seen))
}

pub fn file_format(path: &Path) -> &'static str {
    match path.extension().and_then(|ext| ext.to_str()).map(|ext| ext.to_ascii_lowercase()).as_deref() {
        Some("svg") => "svg",
        Some("png") => "png",
        _ => "other",
    }
}

pub fn describe_icon(name: &str) -> String {
    match locate_icon(name) {
        Some(path) => format!("{name} - {}", file_format(&path)),
        None => format!("{name} - нет"),
    }
}

pub fn chosen_icon_name(kind: &str, on: bool) -> Option<&'static str> {
    icon_candidates(kind, on).iter().copied().find(|name| bundled(name).is_some())
}

fn icon_theme_name() -> String {
    for path in kdeglobals_paths() {
        if let Some(name) = ini_value(&path, "Icons", "Theme") {
            if !name.is_empty() && name != "hicolor" {
                return name;
            }
        }
    }
    let gtk = crate::config::home().join(".config").join("gtk-3.0").join("settings.ini");
    if let Some(name) = ini_value(&gtk, "Settings", "gtk-icon-theme-name") {
        if !name.is_empty() {
            return name;
        }
    }
    "breeze".into()
}

fn kdeglobals_paths() -> [PathBuf; 2] {
    [
        crate::config::config_dir().parent().unwrap_or(Path::new("/")).join("kdeglobals"),
        crate::config::home().join(".config").join("kdeglobals"),
    ]
}

fn icon_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(home) = std::env::var_os("XDG_DATA_HOME") {
        roots.push(PathBuf::from(home).join("icons"));
    }
    roots.push(crate::config::home().join(".local").join("share").join("icons"));
    roots.push(crate::config::home().join(".icons"));
    if let Some(paths) = std::env::var_os("XDG_DATA_DIRS") {
        for dir in std::env::split_paths(&paths) {
            roots.push(dir.join("icons"));
        }
    }
    roots.push(PathBuf::from("/usr/share/icons"));
    roots.push(PathBuf::from("/usr/local/share/icons"));
    roots
}

fn find_icon(theme: &str, name: &str, seen: &mut std::collections::HashSet<String>) -> Option<PathBuf> {
    if theme.is_empty() || !seen.insert(theme.to_string()) {
        return None;
    }
    let root = icon_roots().into_iter().map(|dir| dir.join(theme)).find(|dir| dir.join("index.theme").is_file())?;
    let index = parse_index(&root.join("index.theme"));
    let mut dirs = index.dirs;
    dirs.sort_by_key(|dir| if dir.size == 22 && !dir.scalable { 0 } else if dir.scalable { 80 } else { (dir.size - 22).abs().min(70) });
    for dir in &dirs {
        for ext in ["svg", "png"] {
            let path = root.join(&dir.rel).join(format!("{name}.{ext}"));
            if path.is_file() {
                return Some(path);
            }
        }
    }
    for parent in index.inherits {
        if let Some(found) = find_icon(&parent, name, seen) {
            return Some(found);
        }
    }
    None
}

struct ThemeIndex {
    inherits: Vec<String>,
    dirs: Vec<IconDir>,
}

struct IconDir {
    rel: String,
    size: i32,
    scalable: bool,
}

fn parse_index(path: &Path) -> ThemeIndex {
    let Ok(text) = std::fs::read_to_string(path) else {
        return ThemeIndex { inherits: vec!["hicolor".into()], dirs: Vec::new() };
    };
    let mut section = String::new();
    let mut inherits = Vec::new();
    let mut directories = Vec::new();
    let mut sizes = std::collections::HashMap::<String, (i32, bool)>::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with('[') && line.ends_with(']') {
            section = line[1..line.len() - 1].to_string();
            continue;
        }
        let Some((key, value)) = line.split_once('=') else { continue };
        let key = key.trim();
        let value = value.trim();
        if section == "Icon Theme" && key == "Inherits" {
            inherits = value.split(',').map(|item| item.trim().to_string()).filter(|item| !item.is_empty()).collect();
        } else if section == "Icon Theme" && key == "Directories" {
            directories = value.split(',').map(|item| item.trim().to_string()).filter(|item| !item.is_empty()).collect();
        } else if key == "Size" {
            if let Ok(size) = value.parse() {
                sizes.entry(section.clone()).or_insert((size, false)).0 = size;
            }
        } else if key == "Type" && value.eq_ignore_ascii_case("Scalable") {
            sizes.entry(section.clone()).or_insert((128, true)).1 = true;
        }
    }
    let dirs = directories.into_iter().map(|rel| {
        let (size, scalable) = sizes.get(&rel).copied().unwrap_or((22, false));
        IconDir { rel, size, scalable }
    }).collect();
    if inherits.is_empty() {
        inherits.push("hicolor".into());
    }
    ThemeIndex { inherits, dirs }
}

fn ini_value(path: &Path, section_name: &str, key_name: &str) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut section = String::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with('[') && line.ends_with(']') {
            section = line[1..line.len() - 1].to_string();
            continue;
        }
        if section == section_name {
            if let Some(value) = line.strip_prefix(&format!("{key_name}=")) {
                return Some(value.trim().to_string());
            }
        }
    }
    None
}

fn should_tint(path: &Path) -> bool {
    let Ok(bytes) = std::fs::read(path) else { return false };
    if bytes.starts_with(b"\x89PNG") {
        return false;
    }
    let text = String::from_utf8_lossy(&bytes).to_ascii_lowercase();
    if text.contains("currentcolor") || text.contains("colorscheme-") {
        return true;
    }
    let mut colors = std::collections::HashSet::new();
    for key in ["fill=\"", "fill:", "stroke=\"", "stroke:"] {
        let mut rest = text.as_str();
        while let Some(index) = rest.find(key) {
            rest = &rest[index + key.len()..];
            let color: String = rest.chars().take_while(|c| !matches!(c, '"' | ';' | ' ' | '\n' | ',')).collect();
            if color.is_empty() || color == "none" || color == "transparent" {
                continue;
            }
            colors.insert(color);
        }
    }
    colors.len() <= 1
}

fn turn_on_side(image: slint::Image) -> slint::Image {
    let Some(buffer) = image.to_rgba8() else { return image };
    let width = buffer.width();
    let height = buffer.height();
    if width == 0 || height == 0 {
        return image;
    }
    let src = buffer.as_bytes();
    let mut dst = vec![0u8; src.len()];
    for y in 0..height {
        for x in 0..width {
            let nx = height - 1 - y;
            let ny = x;
            let from = ((y * width + x) * 4) as usize;
            let to = ((ny * height + nx) * 4) as usize;
            dst[to..to + 4].copy_from_slice(&src[from..from + 4]);
        }
    }
    let turned = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(&dst, height, width);
    slint::Image::from_rgba8(turned)
}

fn load_slint_image(path: &Path) -> Option<slint::Image> {
    let bytes = std::fs::read(path).ok()?;
    if bytes.starts_with(b"\x89PNG") {
        return png_image(&bytes);
    }
    if bytes.windows(4).any(|mark| mark == *b"<svg") || bytes.starts_with(b"<?xml") {
        return slint::Image::load_from_svg_data(&bytes).ok();
    }
    slint::Image::load_from_path(path).ok()
}

fn png_image(bytes: &[u8]) -> Option<slint::Image> {
    let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    let pixels = &buf[..info.buffer_size()];
    let rgba = match info.color_type {
        png::ColorType::Rgba => pixels.to_vec(),
        png::ColorType::Rgb => {
            let mut out = Vec::with_capacity(pixels.len() / 3 * 4);
            for px in pixels.chunks_exact(3) {
                out.extend_from_slice(&[px[0], px[1], px[2], 255]);
            }
            out
        }
        _ => return None,
    };
    let buffer = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(&rgba, info.width, info.height);
    Some(slint::Image::from_rgba8(buffer))
}

fn svg_image(bytes: &[u8], edge: u32) -> Option<slint::Image> {
    let tree = resvg::usvg::Tree::from_data(bytes, &resvg::usvg::Options::default()).ok()?;
    let size = tree.size();
    let source = size.width().max(size.height()).max(1.0);
    // Draw at the size the screen will show, so the bitmap is not scaled afterwards.
    let scale = edge.max(1) as f32 / source;
    let width = (size.width() * scale).ceil().max(1.0) as u32;
    let height = (size.height() * scale).ceil().max(1.0) as u32;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)?;
    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());
    let buffer = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(pixmap.data(), width, height);
    Some(slint::Image::from_rgba8(buffer))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_home_assistant_choices_render_without_a_system_theme() {
        for (kind, _) in BUTTON_ICONS {
            for on in [false, true] {
                let icon = button_icon(kind, on, 44).unwrap_or_else(|| panic!("missing {kind}: {on}"));
                assert!(icon.image.size().width > 0);
            }
        }
        assert_eq!(chosen_icon_name("lock", true), Some("object-locked"));
        assert_eq!(chosen_icon_name("lock", false), Some("object-unlocked"));
    }

    #[test]
    fn breeze_pin_is_one_color_and_rasterizes() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("ui/icons/window-pin.svg");
        assert!(should_tint(&path));
        assert!(slint::Image::load_from_svg_data(&std::fs::read(&path).unwrap()).is_ok());
        let file = std::env::temp_dir().join("yoin-colored.svg");
        std::fs::write(&file, "<svg><path fill=\"#ff0000\"/><path fill=\"#00ff00\"/></svg>").unwrap();
        assert!(!should_tint(&file));
        let _ = std::fs::remove_file(file);
    }

    #[test]
    fn svg_raster_matches_the_requested_edge() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("ui/icons/window-pin.svg");
        let bytes = std::fs::read(&path).unwrap();
        let small = svg_image(&bytes, 32).unwrap().to_rgba8().unwrap();
        let large = svg_image(&bytes, 80).unwrap().to_rgba8().unwrap();
        assert_eq!(small.width().max(small.height()), 32);
        assert_eq!(large.width().max(large.height()), 80);
    }

    #[test]
    fn file_format_reads_the_extension() {
        assert_eq!(file_format(Path::new("arrow-up.svg")), "svg");
        assert_eq!(file_format(Path::new("/usr/share/icons/Papirus/22x22/actions/configure.PNG")), "png");
        assert_eq!(file_format(Path::new("icon")), "other");
    }

    #[test]
    fn sideways_icon_turns_clockwise() {
        let mut pixels = vec![0u8; 2 * 2 * 4];
        pixels[0] = 10;
        let image = slint::Image::from_rgba8(slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(&pixels, 2, 2));
        let turned = turn_on_side(image).to_rgba8().unwrap();
        assert_eq!(turned.width(), 2);
        assert_eq!(turned.as_bytes()[4], 10);
    }

    #[test]
    fn switch_uses_the_reverse_icon() {
        assert_eq!(icon_candidates("switch", false), &["reverse"]);
        assert_eq!(icon_candidates("switch", true), icon_candidates("input_boolean", false));
        assert_ne!(icon_candidates("light", false)[0], icon_candidates("light", true)[0]);
    }
}
