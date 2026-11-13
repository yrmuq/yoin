// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-or-later

use std::env;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let cmd = args.next().unwrap_or_default();
    match cmd.as_str() {
        "notes" => notes_cmd(&args.next().unwrap_or_default()),
        "bump" => bump_cmd(&args.next().unwrap_or_default()),
        "embedded" => embedded_cmd(&args.next().unwrap_or_default(), &args.next().unwrap_or_default()),
        "asset-url" => asset_url_cmd(),
        "prerelease-tags" => prerelease_tags_cmd(),
        _ => ExitCode::from(2),
    }
}

fn notes_cmd(kind: &str) -> ExitCode {
    let data = fs::read_to_string("/dev/stdin").unwrap_or_default();
    let text = match kind {
        "deps" => deps_notes(&data),
        "commits" => commit_notes(&data),
        _ => return ExitCode::from(2),
    };
    if text.is_empty() {
        return ExitCode::from(1);
    }
    print!("{text}");
    ExitCode::SUCCESS
}

fn bump_cmd(version: &str) -> ExitCode {
    match bump_version(Path::new("."), version) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::from(1)
        }
    }
}

fn embedded_cmd(path: &str, needle: &str) -> ExitCode {
    if path.is_empty() || needle.is_empty() {
        return ExitCode::from(2);
    }
    if embedded(Path::new(path), needle) { ExitCode::SUCCESS } else { ExitCode::from(1) }
}

fn asset_url_cmd() -> ExitCode {
    let data = fs::read_to_string("/dev/stdin").unwrap_or_default();
    match asset_url(&data) {
        Some(url) => {
            println!("{url}");
            ExitCode::SUCCESS
        }
        None => ExitCode::from(1),
    }
}

fn prerelease_tags_cmd() -> ExitCode {
    let data = fs::read_to_string("/dev/stdin").unwrap_or_default();
    for tag in prerelease_tags(&data) {
        println!("{tag}");
    }
    ExitCode::SUCCESS
}

fn deps_notes(diff: &str) -> String {
    let mut pairs = Vec::new();
    let mut path: Option<String> = None;
    let mut block: Vec<String> = Vec::new();
    let flush = |path: &Option<String>, block: &[String], pairs: &mut Vec<(String, String, String)>| {
        let Some(path) = path else { return };
        if path.ends_with("Cargo.lock") {
            pairs.extend(lock_pairs(block));
        } else if path.ends_with(".yml") || path.ends_with(".yaml") {
            pairs.extend(action_pairs(&body(block, "-"), &body(block, "+")));
        } else if path == "packaging/appimage/build.sh" {
            pairs.extend(tool_pairs(&body(block, "-"), &body(block, "+")));
        }
    };
    for line in diff.split('\n') {
        if let Some(rest) = line.strip_prefix("diff --git ") {
            flush(&path, &block, &mut pairs);
            path = rest.rsplit(" b/").next().map(str::to_string);
            block.clear();
            continue;
        }
        block.push(line.to_string());
    }
    flush(&path, &block, &mut pairs);
    if pairs.is_empty() {
        return String::new();
    }
    pairs.sort();
    pairs.dedup();
    let mut lines = vec!["Dependency updates.".to_string()];
    for (name, old, new) in pairs {
        lines.push(format!("- `{name}` {old} -> {new}"));
    }
    lines.join("\n") + "\n"
}

fn commit_notes(data: &str) -> String {
    let mut added = Vec::new();
    let mut changed = Vec::new();
    let mut fixed = Vec::new();
    let mut removed = Vec::new();
    let mut security = Vec::new();
    for subject in data.split('\n') {
        let Some((section, text)) = classify(subject) else { continue };
        match section {
            "Added" => added.push(text),
            "Changed" => changed.push(text),
            "Fixed" => fixed.push(text),
            "Removed" => removed.push(text),
            "Security" => security.push(text),
            _ => {}
        }
    }
    let mut lines = Vec::new();
    for (name, items) in [("Added", added), ("Changed", changed), ("Fixed", fixed), ("Removed", removed), ("Security", security)] {
        if items.is_empty() {
            continue;
        }
        if !lines.is_empty() {
            lines.push(String::new());
        }
        lines.push(format!("### {name}"));
        for text in items {
            lines.push(format!("- {text}"));
        }
    }
    if lines.is_empty() {
        return String::new();
    }
    lines.join("\n") + "\n"
}

fn classify(subject: &str) -> Option<(&'static str, String)> {
    let trimmed = subject.trim();
    if trimmed.is_empty() {
        return None;
    }
    let lowered = trimmed.to_ascii_lowercase();
    if lowered.starts_with("merge ") || lowered.starts_with("version ") {
        return None;
    }
    if lowered.contains("chmod ") || trimmed.contains("./yoin") || trimmed.contains("Test build") {
        return None;
    }
    let mut text = trimmed.to_string();
    if !text.ends_with('.') {
        text.push('.');
    }
    let section = if word(&lowered, "add") || word(&lowered, "implement") {
        "Added"
    } else if word(&lowered, "fix") || word(&lowered, "resolve") || word(&lowered, "prevent") {
        "Fixed"
    } else if word(&lowered, "remove") {
        "Removed"
    } else if word(&lowered, "redact") || word(&lowered, "restrict") {
        "Security"
    } else {
        "Changed"
    };
    Some((section, text))
}

fn word(lowered: &str, token: &str) -> bool {
    let Some(rest) = lowered.strip_prefix(token) else { return false };
    rest.chars().next().is_none_or(|ch| !ch.is_ascii_alphanumeric())
}

fn body(lines: &[String], prefix: &str) -> Vec<String> {
    lines.iter().filter_map(|line| line.strip_prefix(prefix).map(str::to_string)).collect()
}

fn lock_pairs(lines: &[String]) -> Vec<(String, String, String)> {
    let mut pairs = Vec::new();
    let mut name: Option<String> = None;
    let mut old: Option<String> = None;
    let mut new: Option<String> = None;
    let flush = |name: &mut Option<String>, old: &mut Option<String>, new: &mut Option<String>, pairs: &mut Vec<(String, String, String)>| {
        if let (Some(name), Some(old), Some(new)) = (name.as_ref(), old.as_ref(), new.as_ref()) {
            if name != "yoin" && old != new {
                pairs.push((name.clone(), old.clone(), new.clone()));
            }
        }
        *name = None;
        *old = None;
        *new = None;
    };
    for line in lines {
        if line.is_empty() || line.starts_with("---") || line.starts_with("+++") || line.starts_with("@@") || line.starts_with("index ") || line.starts_with("new file") || line.starts_with("deleted file") {
            continue;
        }
        let prefix = line.as_bytes()[0] as char;
        if prefix != '+' && prefix != '-' && prefix != ' ' {
            continue;
        }
        let text = line[1..].trim();
        if text == "[[package]]" {
            flush(&mut name, &mut old, &mut new, &mut pairs);
            continue;
        }
        if let Some(found) = quoted(text, "name") {
            if name.as_deref().is_some_and(|current| current != found) {
                flush(&mut name, &mut old, &mut new, &mut pairs);
            }
            name = Some(found.to_string());
            continue;
        }
        if let Some(found) = quoted(text, "version") {
            if name.is_some() {
                if prefix == '-' {
                    old = Some(found.to_string());
                } else if prefix == '+' {
                    new = Some(found.to_string());
                }
            }
        }
    }
    flush(&mut name, &mut old, &mut new, &mut pairs);
    pairs
}

fn quoted<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let rest = line.strip_prefix(key)?.trim_start().strip_prefix('=')?.trim_start();
    let rest = rest.strip_prefix('"')?;
    let end = rest.find('"')?;
    Some(&rest[..end])
}

fn action_pairs(minus: &[String], plus: &[String]) -> Vec<(String, String, String)> {
    let old = actions(minus);
    let new = actions(plus);
    let mut pairs = Vec::new();
    for (name, old_ver) in old {
        if let Some(new_ver) = new.iter().find(|(item, _)| item == &name).map(|(_, ver)| ver) {
            if new_ver != &old_ver {
                pairs.push((name, old_ver, new_ver.clone()));
            }
        }
    }
    pairs
}

fn actions(lines: &[String]) -> Vec<(String, String)> {
    let mut found = Vec::new();
    for line in lines {
        if let Some((name, ver)) = action(line) {
            if let Some(slot) = found.iter_mut().find(|(item, _)| item == name) {
                slot.1 = ver.to_string();
            } else {
                found.push((name.to_string(), ver.to_string()));
            }
        }
    }
    found
}

fn action(line: &str) -> Option<(&str, &str)> {
    let at = line.find("uses:")?;
    let rest = line[at + "uses:".len()..].trim();
    let (repo, ver) = rest.split_once('@')?;
    if !repo_ok(repo) || ver.is_empty() || ver.chars().any(char::is_whitespace) {
        return None;
    }
    Some((repo, ver))
}

fn repo_ok(repo: &str) -> bool {
    let Some((owner, name)) = repo.split_once('/') else { return false };
    let piece = |text: &str| !text.is_empty() && text.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '.' || ch == '-');
    piece(owner) && piece(name) && !repo[owner.len() + 1..].contains('/')
}

fn tool_pairs(minus: &[String], plus: &[String]) -> Vec<(String, String, String)> {
    match (tool_ver(minus), tool_ver(plus)) {
        (Some(old), Some(new)) if old != new => vec![("appimagetool".into(), old, new)],
        _ => Vec::new(),
    }
}

fn tool_ver(lines: &[String]) -> Option<String> {
    for line in lines {
        if let Some(ver) = tool_line(line) {
            return Some(ver.to_string());
        }
    }
    None
}

fn tool_line(line: &str) -> Option<&str> {
    let key = "download/";
    let at = line.find(key)?;
    let rest = &line[at + key.len()..];
    let end = rest.find("/appimagetool")?;
    let ver = &rest[..end];
    if ver.is_empty() || ver.contains([' ', '"', '/']) {
        return None;
    }
    Some(ver)
}

fn version_ok(version: &str) -> bool {
    let mut parts = version.split('.');
    let three = [parts.next(), parts.next(), parts.next()];
    parts.next().is_none() && three.iter().all(|part| part.is_some_and(|text| !text.is_empty() && text.chars().all(|ch| ch.is_ascii_digit())))
}

fn bump_version(root: &Path, version: &str) -> Result<(), String> {
    if !version_ok(version) {
        return Err("version is not X.Y.Z".into());
    }
    let cargo_path = root.join("Cargo.toml");
    let lock_path = root.join("Cargo.lock");
    let cargo_text = fs::read_to_string(&cargo_path).map_err(|_| "version line was not found".to_string())?;
    let lock_text = fs::read_to_string(&lock_path).map_err(|_| "version line was not found".to_string())?;
    let (cargo_new, cargo_count) = bump_cargo(&cargo_text, version);
    let (lock_new, lock_count) = bump_lock(&lock_text, version);
    if cargo_count != 1 || lock_count != 1 {
        return Err("version line was not found".into());
    }
    fs::write(&cargo_path, cargo_new).map_err(|err| err.to_string())?;
    fs::write(&lock_path, lock_new).map_err(|err| err.to_string())?;
    Ok(())
}

fn bump_cargo(text: &str, version: &str) -> (String, usize) {
    let mut count = 0;
    let mut out = String::new();
    for line in text.split_inclusive('\n') {
        let body = line.trim_end_matches(['\n', '\r']);
        if count == 0 && body.starts_with("version = \"") && body.ends_with('"') {
            let inside = &body["version = \"".len()..body.len() - 1];
            if version_ok(inside) {
                out.push_str("version = \"");
                out.push_str(version);
                out.push('"');
                out.push_str(&line[body.len()..]);
                count += 1;
                continue;
            }
        }
        out.push_str(line);
    }
    (out, count)
}

fn bump_lock(text: &str, version: &str) -> (String, usize) {
    let mut hit = false;
    let mut count = 0;
    let mut out = String::new();
    for line in text.split_inclusive('\n') {
        let body = line.trim_end_matches(['\n', '\r']);
        if body.trim() == "name = \"yoin\"" {
            hit = true;
        } else if hit && body.starts_with("version = ") {
            if let Some(start) = line.find('"') {
                if let Some(end) = line[start + 1..].find('"') {
                    out.push_str(&line[..start + 1]);
                    out.push_str(version);
                    out.push_str(&line[start + 1 + end..]);
                    hit = false;
                    count += 1;
                    continue;
                }
            }
            hit = false;
        }
        out.push_str(line);
    }
    (out, count)
}

fn embedded(path: &Path, needle: &str) -> bool {
    let Ok(data) = fs::read(path) else {
        eprintln!("update information was not embedded");
        return false;
    };
    let needle = needle.as_bytes();
    if !needle.is_empty() && data.windows(needle.len()).any(|window| window == needle) {
        return true;
    }
    eprintln!("update information was not embedded");
    if let Some(spot) = data.windows(5).position(|window| window == b"zsync") {
        let start = spot.saturating_sub(40);
        let end = (spot + 160).min(data.len());
        eprintln!("{:?}", &data[start..end]);
    }
    false
}

fn asset_url(json: &str) -> Option<String> {
    let mut rest = json;
    while let Some(at) = rest.find("\"browser_download_url\"") {
        let after = &rest[at + "\"browser_download_url\"".len()..];
        rest = after;
        let Some(url) = json_string(after) else { continue };
        let url = url.replace("\\/", "/");
        if url.starts_with("https://github.com/AppImage/appimagetool/releases/download/") && url.ends_with("/appimagetool-x86_64.AppImage") {
            return Some(url);
        }
    }
    None
}

fn json_string(text: &str) -> Option<String> {
    let text = text.trim_start().trim_start_matches(':').trim_start();
    let text = text.strip_prefix('"')?;
    let mut out = String::new();
    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if let Some(next) = chars.next() {
                out.push(next);
            }
            continue;
        }
        if ch == '"' {
            return Some(out);
        }
        out.push(ch);
    }
    None
}

fn prerelease_tags(json: &str) -> Vec<String> {
    let mut tags = Vec::new();
    let mut rest = json;
    while let Some(at) = rest.find("\"tagName\"") {
        let after = &rest[at + "\"tagName\"".len()..];
        rest = after;
        let Some(tag) = json_string(after) else { continue };
        let end = after.find("\"tagName\"").unwrap_or(after.len());
        let window = &after[..end];
        let marked = window.contains("\"isPrerelease\": true") || window.contains("\"isPrerelease\":true");
        if marked {
            tags.push(tag);
        }
    }
    tags
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIFF: &str = "\
diff --git a/Cargo.lock b/Cargo.lock
--- a/Cargo.lock
+++ b/Cargo.lock
@@ -1,8 +1,8 @@
 [[package]]
 name = \"slint\"
-version = \"1.18.0\"
+version = \"1.18.1\"
 [[package]]
 name = \"yoin\"
-version = \"0.12.0\"
+version = \"0.12.1\"
diff --git a/.github/workflows/appimage.yml b/.github/workflows/appimage.yml
--- a/.github/workflows/appimage.yml
+++ b/.github/workflows/appimage.yml
@@ -1 +1 @@
-        uses: actions/checkout@v4
+        uses: actions/checkout@v5
diff --git a/packaging/appimage/build.sh b/packaging/appimage/build.sh
--- a/packaging/appimage/build.sh
+++ b/packaging/appimage/build.sh
@@ -1 +1 @@
-TOOL_URL=\"https://github.com/AppImage/appimagetool/releases/download/1.9.1/appimagetool-x86_64.AppImage\"
+TOOL_URL=\"https://github.com/AppImage/appimagetool/releases/download/1.9.2/appimagetool-x86_64.AppImage\"
";

    #[test]
    fn dependency_notes_name_the_old_and_new_versions() {
        let text = deps_notes(DIFF);
        assert_eq!(
            text,
            "Dependency updates.\n- `actions/checkout` v4 -> v5\n- `appimagetool` 1.9.1 -> 1.9.2\n- `slint` 1.18.0 -> 1.18.1\n"
        );
    }

    #[test]
    fn commit_notes_skip_merges_and_version_bumps() {
        let text = commit_notes("Add a language file.\nFix the seek bar\nMerge branch 'dev'\nVersion 0.13.0.\nMove the pin.\nRun ./yoin after chmod +x.\n");
        assert_eq!(
            text,
            "### Added\n- Add a language file.\n\n### Changed\n- Move the pin.\n\n### Fixed\n- Fix the seek bar.\n"
        );
    }

    #[test]
    fn bump_writes_the_program_version_only() {
        let root = std::env::temp_dir().join(format!("yoin-bump-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("Cargo.toml"), "name = \"yoin\"\nversion = \"0.1.0\"\n\nversion = \"9.9.9\"\n").unwrap();
        fs::write(root.join("Cargo.lock"), "[[package]]\nname = \"yoin\"\nversion = \"0.1.0\"\n[[package]]\nname = \"slint\"\nversion = \"1.0.0\"\n").unwrap();
        bump_version(&root, "0.2.0").unwrap();
        let cargo = fs::read_to_string(root.join("Cargo.toml")).unwrap();
        let lock = fs::read_to_string(root.join("Cargo.lock")).unwrap();
        assert!(cargo.starts_with("name = \"yoin\"\nversion = \"0.2.0\"\n"));
        assert!(cargo.contains("version = \"9.9.9\"\n"));
        assert!(lock.contains("name = \"yoin\"\nversion = \"0.2.0\"\n"));
        assert!(lock.contains("name = \"slint\"\nversion = \"1.0.0\"\n"));
        assert!(bump_version(&root, "nope").is_err());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn asset_url_and_prerelease_tags() {
        let json = r#"{"assets":[{"name":"note.txt","browser_download_url":"https://example.com/note"},{"name":"appimagetool-x86_64.AppImage","browser_download_url":"https://github.com/AppImage/appimagetool/releases/download/1.9.1/appimagetool-x86_64.AppImage"}]}"#;
        assert_eq!(
            asset_url(json).as_deref(),
            Some("https://github.com/AppImage/appimagetool/releases/download/1.9.1/appimagetool-x86_64.AppImage")
        );
        let releases = r#"[{"tagName":"v0.13.0","isPrerelease":false},{"tagName":"v0.10.1","isPrerelease":true}]"#;
        assert_eq!(prerelease_tags(releases), vec!["v0.10.1".to_string()]);
    }
}
