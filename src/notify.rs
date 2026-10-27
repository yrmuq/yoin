// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use zbus::zvariant::Value;

pub async fn announce(title: &str, message: &str, icon_path: &str) -> bool {
    let icon = published_icon(icon_path);
    let Ok(conn) = zbus::Connection::session().await else { return false };
    let hints: HashMap<&str, Value> = HashMap::from([
        ("desktop-entry", Value::from("io.github.yrmuq.yoin")),
        ("urgency", Value::U8(1)),
    ]);
    let actions: Vec<&str> = Vec::new();
    conn.call_method(
        Some("org.freedesktop.Notifications"),
        "/org/freedesktop/Notifications",
        Some("org.freedesktop.Notifications"),
        "Notify",
        &(
            "yoin",
            0u32,
            icon.as_str(),
            title,
            message,
            &actions,
            &hints,
            8000i32,
        ),
    )
    .await
    .is_ok()
}

fn published_icon(icon_path: &str) -> String {
    let source = Path::new(icon_path);
    if icon_path.is_empty() || !source.is_file() {
        return String::new();
    }
    let cache = std::env::var_os("XDG_CACHE_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| crate::config::home().join(".cache"));
    let dest = cache.join("yoin").join("notify.png");
    if let Some(parent) = dest.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let copy = || -> std::io::Result<()> {
        let bytes = fs::read(source)?;
        let tmp = dest.with_extension("png.tmp");
        {
            let mut file = fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o644).open(&tmp)?;
            use std::io::Write;
            file.write_all(&bytes)?;
        }
        fs::rename(tmp, &dest)?;
        Ok(())
    };
    if dest.is_file() {
        let newer = fs::metadata(source).ok().zip(fs::metadata(&dest).ok()).map(|(a, b)| a.modified().ok() > b.modified().ok()).unwrap_or(true);
        if !newer {
            return dest.display().to_string();
        }
    }
    if copy().is_ok() {
        dest.display().to_string()
    } else {
        source.display().to_string()
    }
}
