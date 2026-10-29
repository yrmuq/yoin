// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-or-later

use std::fs;
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const MAX_BUTTONS: usize = 10;
const AUTOSTART_NAME: &str = "io.github.yrmuq.yoin.desktop";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub url: String,
    pub token: String,
    pub device_name: String,
    pub device_id: String,
    pub webhook_id: String,
    pub buttons: Vec<String>,
    pub labels: Vec<String>,
    pub autostart: bool,
    pub language: String,
    pub theme: String,
    pub layout: String,
    pub button_size: u32,
    pub notices_hidden: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            url: String::new(),
            token: String::new(),
            device_name: String::new(),
            device_id: String::new(),
            webhook_id: String::new(),
            buttons: vec![String::new()],
            labels: vec![String::new()],
            autostart: false,
            language: String::new(),
            theme: "system".into(),
            layout: "list".into(),
            button_size: 40,
            notices_hidden: false,
        }
    }
}

pub fn config_dir() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".config"));
    base.join("yoin")
}

pub fn state_dir() -> PathBuf {
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".local").join("state"));
    base.join("yoin")
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.json")
}

pub fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/tmp"))
}

pub fn hostname() -> String {
    fs::read_to_string("/proc/sys/kernel/hostname")
        .ok()
        .and_then(|text| text.split('.').next().map(|s| s.trim().to_string()))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "yoin".into())
}

pub fn normalize(mut cfg: Config) -> Config {
    cfg.url = cfg.url.trim().to_string();
    cfg.token = cfg.token.trim().to_string();
    cfg.device_name = cfg.device_name.trim().to_string();
    cfg.device_id = cfg.device_id.trim().to_string();
    cfg.webhook_id = cfg.webhook_id.trim().to_string();
    cfg.buttons.truncate(MAX_BUTTONS);
    if cfg.buttons.is_empty() {
        cfg.buttons.push(String::new());
    }
    if !matches!(cfg.theme.as_str(), "system" | "light" | "dark") {
        cfg.theme = "system".into();
    }
    if !matches!(cfg.layout.as_str(), "list" | "tiles") {
        cfg.layout = "list".into();
    }
    cfg.button_size = cfg.button_size.clamp(32, 80);
    cfg.labels.truncate(MAX_BUTTONS);
    cfg.labels.resize(cfg.buttons.len(), String::new());
    for label in &mut cfg.labels {
        let trimmed = label.trim();
        *label = trimmed.chars().take(80).collect();
    }
    cfg.language = cfg.language.trim().to_string();
    cfg
}

pub fn align_labels(cfg: &mut Config) {
    cfg.labels.resize(cfg.buttons.len(), String::new());
    cfg.labels.truncate(cfg.buttons.len());
}

pub fn load_config(path: &Path) -> Config {
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(_) => return Config::default(),
    };
    match serde_json::from_str::<Config>(&raw) {
        Ok(cfg) => normalize(cfg),
        Err(_) => {
            let _ = fs::rename(path, path.with_file_name("config.json.bak"));
            Config::default()
        }
    }
}

pub fn save_config(cfg: &Config, path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
        let _ = fs::set_permissions(parent, fs::Permissions::from_mode(0o700));
    }
    let payload = serde_json::to_string_pretty(&normalize(cfg.clone()))? + "\n";
    let tmp = path.with_extension("json.tmp");
    {
        let mut file = fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(&tmp)?;
        file.write_all(payload.as_bytes())?;
    }
    fs::rename(&tmp, path)?;
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    Ok(())
}

pub fn launch_args() -> Vec<String> {
    if std::env::var_os("FLATPAK_ID").is_some() {
        return vec!["flatpak".into(), "run".into(), "io.github.yrmuq.yoin".into()];
    }
    if let Ok(appimage) = std::env::var("APPIMAGE") {
        if !appimage.is_empty() {
            return vec![appimage];
        }
    }
    std::env::current_exe().ok().map(|p| vec![p.display().to_string()]).unwrap_or_else(|| vec!["yoin".into()])
}

fn format_exec(args: &[String]) -> String {
    args.iter()
        .map(|arg| {
            if arg.contains([' ', '\t', '"']) {
                format!("\"{}\"", arg.replace('"', "\\\""))
            } else {
                arg.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn autostart_path() -> PathBuf {
    home().join(".config").join("autostart").join(AUTOSTART_NAME)
}

pub fn apply_autostart(enabled: bool, path: &Path) -> std::io::Result<()> {
    if !enabled {
        if path.is_file() {
            fs::remove_file(path)?;
        }
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = format!(
        "[Desktop Entry]\nType=Application\nName=yoin\nComment=Home Assistant buttons in the KDE tray\nExec={}\nIcon=yoin\nTerminal=false\nCategories=Utility;\nStartupNotify=false\nX-KDE-StartupNotify=false\nX-KDE-autostart-after=panel\nX-GNOME-Autostart-enabled=true\n",
        format_exec(&launch_args())
    );
    fs::write(path, text)?;
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o644));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_configs_gain_language_and_one_button() {
        let cfg: Config = serde_json::from_str(r#"{"url":" http://ha ","token":"t","buttons":[],"theme":"nope"}"#).unwrap();
        let cfg = normalize(cfg);
        assert_eq!(cfg.url, "http://ha");
        assert_eq!(cfg.buttons, vec![String::new()]);
        assert_eq!(cfg.theme, "system");
        assert!(!cfg.autostart);
        assert_eq!(cfg.layout, "list");
        assert_eq!(cfg.button_size, 40);
        assert!(!cfg.notices_hidden);
        assert_eq!(cfg.labels, vec![String::new()]);
    }

    #[test]
    fn labels_follow_buttons_and_size_is_clamped() {
        let cfg: Config = serde_json::from_str(
            r#"{"buttons":["light.a","switch.b"],"labels":[" Lamp "],"layout":"grid","button_size":999}"#,
        )
        .unwrap();
        let cfg = normalize(cfg);
        assert_eq!(cfg.labels, vec!["Lamp".to_string(), String::new()]);
        assert_eq!(cfg.layout, "list");
        assert_eq!(cfg.button_size, 80);
    }
}
