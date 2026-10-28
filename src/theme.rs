// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::Path;

pub fn resolved(choice: &str, system_dark: bool) -> &'static str {
    match choice {
        "light" => "light",
        "dark" => "dark",
        _ => {
            if system_dark { "dark" } else { "light" }
        }
    }
}

pub fn kde_is_dark(text: &str) -> Option<bool> {
    let mut section = String::new();
    let mut scheme = String::new();
    let mut background = String::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with('[') && line.ends_with(']') {
            section = line[1..line.len() - 1].to_string();
            continue;
        }
        let Some((key, value)) = line.split_once('=') else { continue };
        let key = key.trim();
        let value = value.trim();
        if section == "General" && key == "ColorScheme" {
            scheme = value.to_string();
        } else if section == "Colors:Window" && key == "BackgroundNormal" {
            background = value.to_string();
        }
    }
    if !background.is_empty() {
        let parts: Vec<i32> = background.split(',').take(3).map(|p| p.trim().parse().ok()).collect::<Option<_>>()?;
        if parts.len() == 3 {
            let luma = 0.2126 * parts[0] as f32 + 0.7152 * parts[1] as f32 + 0.0722 * parts[2] as f32;
            return Some(luma < 128.0);
        }
    }
    if !scheme.is_empty() {
        return Some(scheme.to_lowercase().contains("dark"));
    }
    None
}

pub fn system_dark() -> bool {
    let path = crate::config::config_dir().parent().unwrap_or(Path::new("/")).join("kdeglobals");
    if let Ok(text) = std::fs::read_to_string(&path) {
        if let Some(dark) = kde_is_dark(&text) {
            return dark;
        }
    }
    let path = crate::config::home().join(".config").join("kdeglobals");
    if let Ok(text) = std::fs::read_to_string(path) {
        if let Some(dark) = kde_is_dark(&text) {
            return dark;
        }
    }
    false
}

pub struct UiFont {
    pub family: String,
    pub size_px: f32,
    pub weight: i32,
}

impl Default for UiFont {
    fn default() -> Self {
        Self {
            family: "Noto Sans".into(),
            size_px: 10.0 * 96.0 / 72.0,
            weight: 400,
        }
    }
}

pub fn font_from_kdeglobals(text: &str) -> Option<UiFont> {
    let mut section = String::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with('[') && line.ends_with(']') {
            section = line[1..line.len() - 1].to_string();
            continue;
        }
        if section != "General" {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else { continue };
        if key.trim() == "font" {
            return parse_qt_font(value.trim());
        }
    }
    None
}

pub fn parse_qt_font(value: &str) -> Option<UiFont> {
    let parts: Vec<&str> = value.split(',').collect();
    let family = parts.first()?.trim().trim_matches('"');
    if family.is_empty() {
        return None;
    }
    let point = parts.get(1).and_then(|s| s.trim().parse::<f32>().ok()).unwrap_or(-1.0);
    let pixel = parts.get(2).and_then(|s| s.trim().parse::<f32>().ok()).unwrap_or(-1.0);
    let qt_weight = parts.get(4).and_then(|s| s.trim().parse::<i32>().ok()).unwrap_or(50);
    let size_px = if point > 0.0 {
        point * 96.0 / 72.0
    } else if pixel > 0.0 {
        pixel
    } else {
        10.0 * 96.0 / 72.0
    };
    Some(UiFont {
        family: family.to_string(),
        size_px,
        weight: qt_weight_to_css(qt_weight),
    })
}

fn qt_weight_to_css(weight: i32) -> i32 {
    match weight {
        i32::MIN..=10 => 100,
        11..=24 => 200,
        25..=49 => 300,
        50..=56 => 400,
        57..=62 => 500,
        63..=74 => 600,
        75..=80 => 700,
        81..=86 => 800,
        _ => 900,
    }
}

pub fn system_font() -> UiFont {
    let paths = [
        crate::config::config_dir().parent().unwrap_or(Path::new("/")).join("kdeglobals"),
        crate::config::home().join(".config").join("kdeglobals"),
    ];
    for path in paths {
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        if let Some(font) = font_from_kdeglobals(&text) {
            return font;
        }
    }
    UiFont::default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kde_background_decides() {
        let text = "[Colors:Window]\nBackgroundNormal=20,20,20\n";
        assert_eq!(kde_is_dark(text), Some(true));
        assert_eq!(resolved("system", true), "dark");
        assert_eq!(resolved("light", true), "light");
    }

    #[test]
    fn qt_font_uses_point_size_and_normal_weight() {
        let text = "[General]\nfont=Noto Sans,10,-1,5,50,0,0,0,0,0,Regular\n";
        let font = font_from_kdeglobals(text).unwrap();
        assert_eq!(font.family, "Noto Sans");
        assert!((font.size_px - (10.0 * 96.0 / 72.0)).abs() < 0.01);
        assert_eq!(font.weight, 400);
        assert_eq!(parse_qt_font("Hack,11,-1,5,75,0,0,0,0,0").unwrap().weight, 700);
    }
}
