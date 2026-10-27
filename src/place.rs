// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-or-later

use std::fs;
use std::path::PathBuf;

const SCRIPT: &str = include_str!("../kwin/placer.js");

pub async fn install() -> bool {
    let cache = std::env::var_os("XDG_CACHE_HOME").map(PathBuf::from).unwrap_or_else(|| crate::config::home().join(".cache"));
    let folder = cache.join("yoin");
    if fs::create_dir_all(&folder).is_err() {
        return false;
    }
    let path = folder.join("placer.js");
    if fs::write(&path, SCRIPT).is_err() {
        return false;
    }
    let Ok(conn) = zbus::Connection::session().await else { return false };
    stop_saved(&conn, &folder.join("placer.id")).await;
    let reply = conn
        .call_method(
            Some("org.kde.KWin"),
            "/Scripting",
            Some("org.kde.kwin.Scripting"),
            "loadScript",
            &(path.display().to_string(), format!("yoinplace{}", std::process::id())),
        )
        .await;
    let Ok(reply) = reply else { return false };
    let Ok(id) = reply.body().deserialize::<i32>() else { return false };
    let object = format!("/Scripting/Script{id}");
    let ran = conn.call_method(Some("org.kde.KWin"), object.as_str(), Some("org.kde.kwin.Script"), "run", &()).await.is_ok();
    if ran {
        let _ = fs::write(folder.join("placer.id"), id.to_string());
    }
    ran
}

async fn stop_saved(conn: &zbus::Connection, path: &std::path::Path) {
    let Ok(text) = fs::read_to_string(path) else { return };
    let Ok(id) = text.trim().parse::<i32>() else { return };
    let object = format!("/Scripting/Script{id}");
    let _ = conn.call_method(Some("org.kde.KWin"), object.as_str(), Some("org.kde.kwin.Script"), "stop", &()).await;
}
