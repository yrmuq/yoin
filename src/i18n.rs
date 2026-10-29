// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
struct Row {
    en: String,
    text: String,
}

#[derive(Clone, Debug)]
pub struct Catalog {
    rows: HashMap<u32, Row>,
}

impl Catalog {
    pub fn load(folder: &Path, name: &str) -> Self {
        let base = read_file(&folder.join("English.ini"));
        let chosen = {
            let trimmed = name.trim();
            if trimmed.is_empty() { "English" } else { trimmed }
        };
        let path = safe_file(folder, chosen);
        let overlay = if let Some(path) = path {
            read_file(&path)
        } else {
            HashMap::new()
        };
        let mut rows = HashMap::new();
        for number in base.keys().chain(overlay.keys()).copied() {
            let original = base.get(&number).map(|r| r.en.clone()).or_else(|| overlay.get(&number).map(|r| r.en.clone())).unwrap_or_default();
            let shown = overlay.get(&number).map(|r| r.text.clone()).filter(|s| !s.is_empty()).unwrap_or_else(|| original.clone());
            rows.insert(number, Row { en: original, text: shown });
        }
        Self { rows }
    }

    pub fn tr(&self, number: u32) -> String {
        self.rows.get(&number).map(|row| if row.text.is_empty() { row.en.clone() } else { row.text.clone() }).unwrap_or_else(|| number.to_string())
    }

    pub fn trf(&self, number: u32, args: &[&str]) -> String {
        let Some(row) = self.rows.get(&number) else { return number.to_string() };
        let pattern = if row.text.matches("{}").count() == row.en.matches("{}").count() { &row.text } else { &row.en };
        fill(pattern, args).unwrap_or_else(|| fill(&row.en, args).unwrap_or_else(|| row.en.clone()))
    }
}

pub fn languages(folder: &Path) -> Vec<String> {
    let mut names = Vec::new();
    if let Ok(rd) = std::fs::read_dir(folder) {
        for entry in rd.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("ini") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    names.push(stem.to_string());
                }
            }
        }
    }
    names.sort();
    if let Some(pos) = names.iter().position(|n| n == "English") {
        names.remove(pos);
    }
    names.insert(0, "English".into());
    names
}

pub fn match_language(folder: &Path, ui_languages: &[String]) -> String {
    let mut codes = HashMap::new();
    for name in languages(folder) {
        let code = code_of(folder, &name);
        if !code.is_empty() && !codes.contains_key(&code) {
            codes.insert(code, name);
        }
    }
    for item in ui_languages {
        let token = item.replace('-', "_");
        let token = token.split('.').next().unwrap_or("");
        let lang = token.split('_').next().unwrap_or("").to_lowercase();
        if let Some(name) = codes.get(&lang) {
            return name.clone();
        }
    }
    "English".into()
}

pub fn system_languages() -> Vec<String> {
    let raw = std::env::var("LANGUAGE").ok().filter(|s| !s.is_empty()).or_else(|| std::env::var("LC_ALL").ok()).or_else(|| std::env::var("LC_MESSAGES").ok()).or_else(|| std::env::var("LANG").ok()).unwrap_or_default();
    raw.split(':').filter(|s| !s.is_empty()).map(|s| s.to_string()).collect()
}

fn fill(pattern: &str, args: &[&str]) -> Option<String> {
    let mut out = String::new();
    let mut rest = pattern;
    let mut index = 0;
    while let Some(pos) = rest.find("{}") {
        out.push_str(&rest[..pos]);
        out.push_str(args.get(index)?);
        index += 1;
        rest = &rest[pos + 2..];
    }
    if index != args.len() && pattern.matches("{}").count() != args.len() {
        return None;
    }
    out.push_str(rest);
    Some(out)
}

fn safe_file(folder: &Path, name: &str) -> Option<PathBuf> {
    if name.is_empty() || name == "." || name == ".." || name.contains('/') || name.contains('\\') {
        return None;
    }
    let root = folder.canonicalize().ok()?;
    let path = root.join(format!("{name}.ini"));
    if path.parent()? != root || !path.is_file() {
        return None;
    }
    Some(path)
}

fn code_of(folder: &Path, name: &str) -> String {
    let Some(path) = safe_file(folder, name) else { return String::new() };
    let Ok(text) = std::fs::read_to_string(path) else { return String::new() };
    for line in text.lines() {
        let text = line.trim();
        if let Some(rest) = text.trim_start_matches(';').trim().strip_prefix("code") {
            if let Some(value) = rest.trim().strip_prefix('=') {
                return value.trim().to_lowercase();
            }
        }
    }
    String::new()
}

fn read_file(path: &Path) -> HashMap<u32, Row> {
    let mut table = HashMap::new();
    let Ok(text) = std::fs::read_to_string(path) else { return table };
    let text = text.trim_start_matches('\u{feff}');
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(['#', ';', '[']) || line.matches('=').count() < 2 {
            continue;
        }
        let Some((left, rest)) = line.split_once('=') else { continue };
        let Some((original, shown)) = rest.rsplit_once('=') else { continue };
        let Ok(number) = left.trim().parse::<u32>() else { continue };
        let original = original.trim().to_string();
        let shown = shown.trim();
        let text = if shown.is_empty() { original.clone() } else { shown.to_string() };
        table.insert(number, Row { en: original, text });
    }
    table
}

pub fn language_dir() -> PathBuf {
    if let Ok(env) = std::env::var("YOIN_LANGUAGES") {
        if !env.is_empty() {
            return PathBuf::from(env);
        }
    }
    if Path::new("languages").is_dir() {
        return PathBuf::from("languages");
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let share = dir.join("../share/yoin/languages");
            if share.is_dir() {
                return share;
            }
            let next = dir.join("languages");
            if next.is_dir() {
                return next;
            }
        }
    }
    PathBuf::from("languages")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("languages")
    }

    #[test]
    fn russian_matches_and_keeps_placeholders() {
        let folder = dir();
        assert_eq!(match_language(&folder, &["ru_RU.UTF-8".into()]), "Russian");
        assert_eq!(match_language(&folder, &["fr".into()]), "English");
        let cat = Catalog::load(&folder, "Russian");
        assert_eq!(cat.tr(2), "Выход");
        assert!(cat.trf(29, &["404"]).contains("404"));
        assert_eq!(cat.trf(29, &["404"]).matches("{}").count(), 0);
    }
}
