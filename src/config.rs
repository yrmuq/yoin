// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-or-later

use std::fs;
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const MAX_BUTTONS: usize = 10;
pub const DEFAULT_SETTINGS_WIDTH: u32 = 600;
pub const DEFAULT_SETTINGS_HEIGHT: u32 = 630;
pub const DEFAULT_LOGO_SIZE: u32 = 100;
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
    pub icons: Vec<String>,
    pub lock_window_size: bool,
    pub autostart: bool,
    pub auto_check_updates: bool,
    pub language: String,
    pub theme: String,
    pub layout: String,
    pub button_size: u32,
    pub notices_hidden: bool,
    pub notice_limit: u32,
    #[serde(skip_serializing_if = "is_default_settings_width")]
    pub settings_width: u32,
    #[serde(skip_serializing_if = "is_default_settings_height")]
    pub settings_height: u32,
    #[serde(skip_serializing_if = "is_default_logo_size")]
    pub logo_size: u32,
    #[serde(default, skip_serializing_if = "is_false")]
    pub dev_tab: bool,
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
            icons: vec![String::new()],
            lock_window_size: true,
            autostart: false,
            auto_check_updates: true,
            language: String::new(),
            theme: "system".into(),
            layout: "list".into(),
            button_size: 40,
            notices_hidden: false,
            notice_limit: 3,
            settings_width: DEFAULT_SETTINGS_WIDTH,
            settings_height: DEFAULT_SETTINGS_HEIGHT,
            logo_size: DEFAULT_LOGO_SIZE,
            dev_tab: false,
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
    cfg.notice_limit = cfg.notice_limit.min(11);
    cfg.settings_width = cfg.settings_width.clamp(360, 2200);
    cfg.settings_height = cfg.settings_height.clamp(240, 1800);
    cfg.logo_size = cfg.logo_size.clamp(32, 256);
    align_buttons(&mut cfg);
    for icon in &mut cfg.icons {
        if !crate::icons::BUTTON_ICONS.iter().any(|(kind, _)| *kind == icon.as_str()) {
            icon.clear();
        }
    }
    for label in &mut cfg.labels {
        let trimmed = label.trim();
        *label = trimmed.chars().take(80).collect();
    }
    cfg.language = cfg.language.trim().to_string();
    cfg
}

fn is_default_settings_width(value: &u32) -> bool {
    *value == DEFAULT_SETTINGS_WIDTH
}

fn is_default_settings_height(value: &u32) -> bool {
    *value == DEFAULT_SETTINGS_HEIGHT
}

fn is_default_logo_size(value: &u32) -> bool {
    *value == DEFAULT_LOGO_SIZE
}

fn is_false(value: &bool) -> bool {
    !*value
}

pub fn align_buttons(cfg: &mut Config) {
    cfg.labels.resize(cfg.buttons.len(), String::new());
    cfg.labels.truncate(cfg.buttons.len());
    cfg.icons.resize(cfg.buttons.len(), String::new());
    cfg.icons.truncate(cfg.buttons.len());
}

pub fn move_button(cfg: &mut Config, from: usize, to: usize) -> bool {
    if from >= cfg.buttons.len() || to >= cfg.buttons.len() || from == to {
        return false;
    }
    align_buttons(cfg);
    let entity = cfg.buttons.remove(from);
    let label = cfg.labels.remove(from);
    let icon = cfg.icons.remove(from);
    cfg.buttons.insert(to, entity);
    cfg.labels.insert(to, label);
    cfg.icons.insert(to, icon);
    true
}

pub fn record_window_size(cfg: &mut Config, width: u32, height: u32) -> bool {
    if cfg.lock_window_size { return false; }
    let size = (width.clamp(360, 2200), height.clamp(240, 1800));
    if (cfg.settings_width, cfg.settings_height) == size { return false; }
    (cfg.settings_width, cfg.settings_height) = size;
    true
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
        assert_eq!(cfg.notice_limit, 3);
        assert_eq!(cfg.labels, vec![String::new()]);
        assert_eq!((cfg.settings_width, cfg.settings_height, cfg.logo_size), (600, 630, 100));
        assert!(cfg.lock_window_size);
        assert!(cfg.auto_check_updates);
        assert_eq!(cfg.icons, vec![String::new()]);
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

    #[test]
    fn drag_moves_the_entity_label_and_icon_together() {
        let mut cfg = normalize(serde_json::from_str(r#"{"buttons":["light.a","switch.b","script.c"],"labels":["Lamp","Plug","Away"],"icons":["light","valve","automation"]}"#).unwrap());
        assert!(move_button(&mut cfg, 0, 2));
        assert_eq!(cfg.buttons, ["switch.b", "script.c", "light.a"]);
        assert_eq!(cfg.labels, ["Plug", "Away", "Lamp"]);
        assert_eq!(cfg.icons, ["valve", "automation", "light"]);
        assert!(move_button(&mut cfg, 2, 0));
        assert_eq!(cfg.labels, ["Lamp", "Plug", "Away"]);
        assert!(!move_button(&mut cfg, usize::MAX, 0));
        assert!(!move_button(&mut cfg, 0, 3));
        let restored = normalize(serde_json::from_str(&serde_json::to_string(&cfg).unwrap()).unwrap());
        assert_eq!(restored, cfg);
    }

    #[test]
    fn icon_choices_and_window_lock_survive_config_updates() {
        let mut cfg = normalize(serde_json::from_str(r#"{"buttons":["light.a","switch.b"],"icons":["../../other.svg","fan","unused"]}"#).unwrap());
        assert_eq!(cfg.icons, ["", "fan"]);
        assert!(!record_window_size(&mut cfg, 800, 700));
        cfg.lock_window_size = false;
        cfg.auto_check_updates = false;
        assert!(record_window_size(&mut cfg, 800, 700));
        assert!(!record_window_size(&mut cfg, 800, 700));
        let restored = normalize(serde_json::from_str(&serde_json::to_string(&cfg).unwrap()).unwrap());
        assert_eq!((restored.settings_width, restored.settings_height), (800, 700));
        assert!(!restored.lock_window_size);
        assert!(!restored.auto_check_updates);
    }

    #[test]
    fn notice_limit_defaults_to_three_and_stops_at_eleven() {
        let cfg: Config = serde_json::from_str(r#"{"notice_limit":40}"#).unwrap();
        let cfg = normalize(cfg);
        assert_eq!(cfg.notice_limit, 11);
        let cfg: Config = serde_json::from_str(r#"{"notice_limit":0}"#).unwrap();
        assert_eq!(normalize(cfg).notice_limit, 0);
    }

    #[test]
    fn dev_fields_stay_out_of_the_file_until_they_change() {
        let json = serde_json::to_string(&normalize(Config::default())).unwrap();
        assert!(!json.contains("settings_width"));
        assert!(!json.contains("settings_height"));
        assert!(!json.contains("logo_size"));
        assert!(!json.contains("dev_tab"));
        let mut cfg = Config::default();
        cfg.settings_width = 700;
        cfg.dev_tab = true;
        let json = serde_json::to_string(&normalize(cfg)).unwrap();
        assert!(json.contains("\"settings_width\":700"));
        assert!(json.contains("\"dev_tab\":true"));
        let cfg: Config = serde_json::from_str(
            r#"{"settings_width":10,"settings_height":5000,"logo_size":1}"#,
        )
        .unwrap();
        let cfg = normalize(cfg);
        assert_eq!(cfg.settings_width, 360);
        assert_eq!(cfg.settings_height, 1800);
        assert_eq!(cfg.logo_size, 32);
    }
}
