// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-or-later

use std::fs;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::config;
use crate::i18n::Catalog;

pub const LIMIT: usize = 3;

#[derive(Clone, Debug, PartialEq)]
pub struct Note {
    pub title: String,
    pub message: String,
    pub at: i64,
    pub image: String,
    pub fade: f32,
    pub closing: bool,
}

#[derive(Serialize, Deserialize)]
struct Stored {
    title: String,
    message: String,
    at: i64,
    image: String,
}

pub fn path() -> PathBuf {
    config::state_dir().join("notices.json")
}

pub fn shots_dir() -> PathBuf {
    config::state_dir().join("shots")
}

pub fn load() -> Vec<Note> {
    let Ok(raw) = fs::read_to_string(path()) else { return Vec::new() };
    let Ok(rows) = serde_json::from_str::<Vec<Stored>>(&raw) else { return Vec::new() };
    rows.into_iter().map(from_stored).filter(|note| note.at > 0).collect()
}

pub fn save(notes: &[Note]) {
    let rows: Vec<Stored> = notes.iter().map(to_stored).collect();
    let Ok(payload) = serde_json::to_string_pretty(&rows) else { return };
    let path = path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let tmp = path.with_extension("json.tmp");
    let wrote = (|| -> std::io::Result<()> {
        let mut file = fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(&tmp)?;
        file.write_all(payload.as_bytes())?;
        file.write_all(b"\n")?;
        Ok(())
    })();
    if wrote.is_ok() {
        let _ = fs::rename(&tmp, &path);
    }
}

/// Keeps the newest three. Returns image files that are no longer used.
pub fn push(notes: &mut Vec<Note>, note: Note) -> Vec<String> {
    notes.push(note);
    if notes.len() <= LIMIT {
        return Vec::new();
    }
    let extra = notes.len() - LIMIT;
    let dropped: Vec<Note> = notes.drain(0..extra).collect();
    dropped.into_iter().map(|note| note.image).filter(|path| !path.is_empty()).collect()
}

pub fn forget(paths: &[String]) {
    let root = shots_dir();
    for path in paths {
        let path = Path::new(path);
        if path.starts_with(&root) {
            let _ = fs::remove_file(path);
        }
    }
}

pub fn dismiss(notes: &mut Vec<Note>, index: usize) -> Option<String> {
    if index >= notes.len() {
        return None;
    }
    Some(notes.remove(index).image)
}

pub fn attach(notes: &mut [Note], at: i64, image: String) -> bool {
    let Some(note) = notes.iter_mut().find(|note| note.at == at) else { return false };
    note.image = image;
    true
}

pub fn keep_shot(source: &Path) -> Option<String> {
    if !source.is_file() {
        return None;
    }
    let ext = source.extension().and_then(|ext| ext.to_str()).unwrap_or("img");
    if ext.contains('/') || ext.contains('.') || ext.len() > 5 {
        return None;
    }
    let dir = shots_dir();
    fs::create_dir_all(&dir).ok()?;
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_millis();
    let dest = dir.join(format!("shot-{stamp}.{ext}"));
    fs::copy(source, &dest).ok()?;
    Some(dest.display().to_string())
}

pub fn ago(at: i64, now: i64, language: &str, catalog: &Catalog) -> String {
    let secs = now.saturating_sub(at).max(0);
    if secs < 60 {
        return catalog.tr(40);
    }
    if secs < 3600 {
        return counted(secs / 60, language, catalog, 41);
    }
    if secs < 24 * 3600 {
        return counted(secs / 3600, language, catalog, 44);
    }
    let days = civil_day(now) - civil_day(at);
    if days <= 1 {
        return catalog.tr(47);
    }
    counted(days.max(2) as i64, language, catalog, 48)
}

fn counted(n: i64, language: &str, catalog: &Catalog, one: u32) -> String {
    let form = plural_form(language, n.max(0) as u64);
    let id = one + form as u32;
    catalog.trf(id, &[&n.to_string()])
}

fn plural_form(language: &str, n: u64) -> usize {
    if language.eq_ignore_ascii_case("russian") {
        let tail = n % 100;
        if (11..=14).contains(&tail) {
            return 2;
        }
        return match tail % 10 {
            1 => 0,
            2..=4 => 1,
            _ => 2,
        };
    }
    if n == 1 { 0 } else { 2 }
}

fn from_stored(row: Stored) -> Note {
    let image = shot_file(&row.image);
    Note {
        title: row.title,
        message: row.message,
        at: row.at,
        image: if image.is_file() { image.display().to_string() } else { String::new() },
        fade: 1.0,
        closing: false,
    }
}

fn to_stored(note: &Note) -> Stored {
    let name = Path::new(&note.image)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.contains("..") && Path::new(&note.image).starts_with(shots_dir()))
        .unwrap_or("")
        .to_string();
    Stored { title: note.title.clone(), message: note.message.clone(), at: note.at, image: name }
}

fn shot_file(name: &str) -> PathBuf {
    if name.is_empty() || name.contains('/') || name.contains('\\') || name.contains("..") {
        return PathBuf::new();
    }
    shots_dir().join(name)
}

fn civil_day(unix: i64) -> i64 {
    let t = unix as libc::time_t;
    let mut broken: libc::tm = unsafe { std::mem::zeroed() };
    let ptr = unsafe { libc::localtime_r(&t, &mut broken) };
    if ptr.is_null() {
        return unix.div_euclid(86400);
    }
    days_from_civil(broken.tm_year + 1900, broken.tm_mon + 1, broken.tm_mday)
}

fn days_from_civil(year: i32, month: i32, day: i32) -> i64 {
    let year = year as i64 - if month <= 2 { 1 } else { 0 };
    let era = year.div_euclid(400);
    let yoe = (year - era * 400) as u64;
    let month = month as i64;
    let doy = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy as u64;
    era * 146097 + doe as i64 - 719468
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog() -> Catalog {
        Catalog::load(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("languages"), "Russian")
    }

    #[test]
    fn russian_age_uses_three_forms() {
        let cat = catalog();
        let now = 1_700_000_000;
        assert_eq!(ago(now - 30, now, "Russian", &cat), "Только что");
        assert_eq!(ago(now - 5 * 60, now, "Russian", &cat), "5 минут назад");
        assert_eq!(ago(now - 2 * 60, now, "Russian", &cat), "2 минуты назад");
        assert_eq!(ago(now - 21 * 60, now, "Russian", &cat), "21 минуту назад");
        assert_eq!(ago(now - 3 * 3600, now, "Russian", &cat), "3 часа назад");
        let mut yesterday = now - 26 * 3600;
        for _ in 0..48 {
            if civil_day(now) - civil_day(yesterday) == 1 && now - yesterday >= 24 * 3600 {
                break;
            }
            yesterday += 1800;
        }
        assert_eq!(civil_day(now) - civil_day(yesterday), 1);
        assert_eq!(ago(yesterday, now, "Russian", &cat), "вчера");
        let older = now - 5 * 86400;
        let days = civil_day(now) - civil_day(older);
        assert_eq!(ago(older, now, "Russian", &cat), counted(days, "Russian", &cat, 48));
    }

    #[test]
    fn fourth_notice_drops_the_oldest_picture() {
        let mut notes = Vec::new();
            let mut dropped = Vec::new();
            for n in 0..4 {
                dropped.extend(push(&mut notes, Note {
                    title: n.to_string(),
                    message: String::new(),
                    at: n,
                    image: if n == 0 { "old.jpg".into() } else { String::new() },
                    fade: 1.0,
                    closing: false,
                }));
            }
        assert_eq!(notes.len(), 3);
        assert_eq!(notes[0].title, "1");
        assert_eq!(dropped, vec!["old.jpg".to_string()]);
    }

    #[test]
    fn dismiss_removes_that_notice_only() {
        let mut notes = vec![
            Note { title: "a".into(), message: String::new(), at: 1, image: "a.jpg".into(), fade: 1.0, closing: false },
            Note { title: "b".into(), message: String::new(), at: 2, image: String::new(), fade: 1.0, closing: false },
        ];
        assert_eq!(dismiss(&mut notes, 0).as_deref(), Some("a.jpg"));
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].title, "b");
        assert!(dismiss(&mut notes, 4).is_none());
    }
}
