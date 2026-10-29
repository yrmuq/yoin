// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use zbus::zvariant::Value;

pub async fn announce(title: &str, message: &str, icon_path: &str, image_path: Option<&str>) -> bool {
    let icon = published_icon(icon_path);
    let image = image_path.unwrap_or("").to_string();
    let Ok(conn) = zbus::Connection::session().await else { return false };
    let mut hints: HashMap<&str, Value> = HashMap::from([
        ("desktop-entry", Value::from("io.github.yrmuq.yoin")),
        ("urgency", Value::U8(1)),
    ]);
    if !image.is_empty() {
        hints.insert("image-path", Value::from(image.as_str()));
        hints.insert("image_path", Value::from(image.as_str()));
    }
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

pub async fn fetch_image(base: &str, token: &str, raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    if Path::new(raw).is_file() {
        return Some(raw.to_string());
    }
    let url = resolve_url(base, raw)?;
    let client = reqwest::Client::builder().timeout(Duration::from_secs(8)).use_rustls_tls().build().ok()?;
    let mut request = client.get(&url);
    if same_host(base, &url) && !token.is_empty() {
        request = request.bearer_auth(token);
    }
    let response = request.send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }
    if response.content_length().unwrap_or(0) > 8 * 1024 * 1024 {
        return None;
    }
    let bytes = response.bytes().await.ok()?;
    if bytes.len() < 32 || bytes.len() > 8 * 1024 * 1024 {
        return None;
    }
    let ext = sniff(&bytes)?;
    store_shot(&bytes, ext)
}

fn resolve_url(base: &str, raw: &str) -> Option<String> {
    if raw.starts_with("https://") || raw.starts_with("http://") {
        return Some(raw.to_string());
    }
    if raw.starts_with('/') {
        let base = base.trim().trim_end_matches('/');
        if base.starts_with("http://") || base.starts_with("https://") {
            return Some(format!("{base}{raw}"));
        }
    }
    None
}

fn same_host(base: &str, url: &str) -> bool {
    host_of(base).is_some_and(|left| host_of(url) == Some(left))
}

fn host_of(url: &str) -> Option<&str> {
    let rest = url.split_once("://")?.1;
    let hostport = rest.split(['/', '?', '#']).next()?;
    Some(hostport.rsplit('@').next()?.split(':').next()?)
}

fn sniff(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some("jpg");
    }
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        return Some("png");
    }
    if bytes.len() > 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return Some("webp");
    }
    if bytes.starts_with(b"GIF8") {
        return Some("gif");
    }
    None
}

fn shot_dir() -> PathBuf {
    let cache = std::env::var_os("XDG_CACHE_HOME").map(PathBuf::from).unwrap_or_else(|| crate::config::home().join(".cache"));
    cache.join("yoin").join("shots")
}

fn store_shot(bytes: &[u8], ext: &str) -> Option<String> {
    let dir = shot_dir();
    fs::create_dir_all(&dir).ok()?;
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_millis();
    let dest = dir.join(format!("shot-{}-{stamp}.{ext}", std::process::id()));
    let tmp = dest.with_extension(format!("{ext}.tmp"));
    {
        let mut file = fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o644).open(&tmp).ok()?;
        file.write_all(bytes).ok()?;
    }
    fs::rename(&tmp, &dest).ok()?;
    trim_shots(&dir);
    Some(dest.display().to_string())
}

fn trim_shots(dir: &Path) {
    let mut files: Vec<_> = fs::read_dir(dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|item| item.ok())
        .filter(|item| item.path().is_file())
        .filter_map(|item| {
            let modified = item.metadata().ok().and_then(|meta| meta.modified().ok())?;
            Some((modified, item.path()))
        })
        .collect();
    if files.len() <= 12 {
        return;
    }
    files.sort_by_key(|(modified, _)| *modified);
    for (_, path) in files.into_iter().rev().skip(12) {
        let _ = fs::remove_file(path);
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_url_stays_on_the_home_assistant_host() {
        assert_eq!(
            resolve_url("http://ha.local:8123/", "/api/camera_proxy/camera.a").as_deref(),
            Some("http://ha.local:8123/api/camera_proxy/camera.a")
        );
        assert!(same_host("http://ha.local:8123", "http://ha.local:8123/api/x"));
        assert!(!same_host("http://ha.local:8123", "https://cdn.example/a.jpg"));
        assert_eq!(sniff(&[0xFF, 0xD8, 0xFF, 0x00]), Some("jpg"));
        assert_eq!(sniff(b"not an image"), None);
    }
}