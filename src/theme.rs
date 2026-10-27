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
}
