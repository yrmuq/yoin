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
    let rules_changed = write_taskbar_rule();
    if rules_changed {
        let _ = conn.call_method(Some("org.kde.KWin"), "/KWin", Some("org.kde.KWin"), "reconfigure", &()).await;
    }
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

const TASKBAR_RULE: &str = "\
[yoin-tray]
Description=yoin tray popup
skiptaskbar=true
skiptaskbarrule=2
skippager=true
skippagerrule=2
skipswitcher=true
skipswitcherrule=2
title=yoin-buttons
titlematch=1
wmclass=yoin
wmclassmatch=1
wmclasscomplete=false
";

/// KWin applies window rules while the window is created, before the task
/// manager can show an icon. Returns true when the file changed.
fn write_taskbar_rule() -> bool {
    let path = crate::config::home().join(".config").join("kwinrulesrc");
    let previous = fs::read_to_string(&path).unwrap_or_default();
    let Some(next) = merge_kwin_rules(&previous) else { return false };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    fs::write(path, next).is_ok()
}

pub async fn raise_settings() {
    let Ok(conn) = zbus::Connection::session().await else { return };
    let _ = conn
        .call_method(
            Some("org.kde.kglobalaccel"),
            "/component/kwin",
            Some("org.kde.kglobalaccel.Component"),
            "invokeShortcut",
            &("Yoin Raise Settings",),
        )
        .await;
}

fn rewrite_tray_rule(text: &str) -> String {
    let Some(start) = text.find("[yoin-tray]") else { return text.to_string() };
    let after = &text[start + "[yoin-tray]".len()..];
    let end = after.find("\n[").map(|index| start + "[yoin-tray]".len() + index + 1).unwrap_or(text.len());
    let mut out = String::new();
    out.push_str(&text[..start]);
    out.push_str(TASKBAR_RULE);
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(text[end..].trim_start_matches('\n'));
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}
fn merge_kwin_rules(text: &str) -> Option<String> {
    if text.contains("[yoin-tray]") {
        if text.contains("title=yoin-buttons\n") {
            return None;
        }
        return Some(rewrite_tray_rule(text));
    }
    let mut out = String::new();
    let mut in_general = false;
    let mut saw_general = false;
    let mut saw_rules = false;
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim().trim_end_matches('\r');
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            if in_general && !saw_rules {
                out.push_str("rules=yoin-tray\n");
                saw_rules = true;
            }
            in_general = trimmed == "[General]";
            saw_general |= in_general;
            out.push_str(line);
            continue;
        }
        if in_general {
            if let Some(rest) = trimmed.strip_prefix("rules=") {
                let rest = rest.trim();
                let value = if rest.is_empty() { "yoin-tray".to_string() } else { format!("{rest},yoin-tray") };
                out.push_str("rules=");
                out.push_str(&value);
                out.push('\n');
                saw_rules = true;
                continue;
            }
            if let Some(rest) = trimmed.strip_prefix("count=") {
                if let Ok(count) = rest.trim().parse::<u32>() {
                    out.push_str(&format!("count={}\n", count + 1));
                    continue;
                }
            }
        }
        out.push_str(line);
    }
    if in_general && !saw_rules {
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str("rules=yoin-tray\n");
    }
    if !saw_general {
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str("[General]\nrules=yoin-tray\ncount=1\n");
    }
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out.push('\n');
    out.push_str(TASKBAR_RULE);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kwin_rule_is_inserted_once() {
        let first = merge_kwin_rules("[General]\nrules=1\ncount=1\n\n[1]\nwmclass=kate\n").unwrap();
        assert!(first.contains("rules=1,yoin-tray\n"));
        assert!(first.contains("count=2\n"));
        assert!(first.contains("skiptaskbar=true\n"));
        assert!(first.contains("title=yoin-buttons\n"));
        assert!(merge_kwin_rules(&first).is_none());
        let stale = "[General]\nrules=yoin-tray\ncount=1\n\n[yoin-tray]\nwmclass=yoin\nskiptaskbar=true\n";
        let updated = merge_kwin_rules(stale).unwrap();
        assert!(updated.contains("title=yoin-buttons\n"));
        assert!(!updated.contains("Description=yoin tray windows\n"));
        assert!(merge_kwin_rules(&updated).is_none());
        let empty = merge_kwin_rules("").unwrap();
        assert!(empty.contains("[yoin-tray]\n"));
        assert!(empty.contains("rules=yoin-tray\n"));
    }
}
