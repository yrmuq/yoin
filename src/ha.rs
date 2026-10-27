// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-or-later

use serde_json::Value;

pub const APP_ID: &str = "io.github.yrmuq.yoin";
pub const APP_NAME: &str = "yoin";

const TOGGLE: &[&str] = &[
    "automation",
    "cover",
    "fan",
    "group",
    "humidifier",
    "input_boolean",
    "light",
    "media_player",
    "remote",
    "siren",
    "switch",
    "valve",
];
const POWER: &[&str] = &["climate", "water_heater"];
const QUIET: &[&str] = &[
    "",
    "off",
    "unknown",
    "unavailable",
    "disarmed",
    "unlocked",
    "closed",
    "standby",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entity {
    pub entity_id: String,
    pub name: String,
    pub domain: String,
    pub state: String,
}

pub fn ws_url(url: &str) -> Result<String, ()> {
    let base = url.trim().trim_end_matches('/');
    if let Some(rest) = base.strip_prefix("https://") {
        return Ok(format!("wss://{rest}/api/websocket"));
    }
    if let Some(rest) = base.strip_prefix("http://") {
        return Ok(format!("ws://{rest}/api/websocket"));
    }
    Err(())
}

pub fn http_url(url: &str, path: &str) -> String {
    format!("{}{path}", url.trim().trim_end_matches('/'))
}

pub fn registration_body(device_id: &str, device_name: &str, version: &str, os_version: &str) -> Value {
    serde_json::json!({
        "device_id": device_id,
        "app_id": APP_ID,
        "app_name": APP_NAME,
        "app_version": version,
        "device_name": device_name,
        "manufacturer": "PC",
        "model": "KDE",
        "os_name": "Linux",
        "os_version": os_version,
        "supports_encryption": false,
        "app_data": {"push_websocket_channel": true},
    })
}

pub fn action_for(entity_id: &str, state: &str) -> Option<(&'static str, String)> {
    let (domain, _) = entity_id.split_once('.')?;
    if domain.is_empty() {
        return None;
    }
    if TOGGLE.contains(&domain) {
        return Some((domain_static(domain)?, "toggle".into()));
    }
    if domain == "script" {
        return Some(("script", "turn_on".into()));
    }
    if domain == "lock" {
        let service = if state == "locked" { "unlock" } else { "lock" };
        return Some(("lock", service.into()));
    }
    if domain == "alarm_control_panel" {
        let armed = state.starts_with("armed") || matches!(state, "arming" | "pending" | "triggered");
        let service = if armed { "alarm_disarm" } else { "alarm_arm_away" };
        return Some(("alarm_control_panel", service.into()));
    }
    if POWER.contains(&domain) {
        let service = if state == "off" { "turn_on" } else { "turn_off" };
        return Some((domain_static(domain)?, service.into()));
    }
    None
}

fn domain_static(domain: &str) -> Option<&'static str> {
    TOGGLE.iter().chain(POWER.iter()).copied().find(|item| *item == domain)
}

pub fn notification_text(event: &Value) -> Option<(String, String)> {
    let event = event.as_object()?;
    if !event.contains_key("title") && !event.contains_key("message") {
        return None;
    }
    let title = event.get("title").and_then(Value::as_str).unwrap_or("").trim();
    let message = event.get("message").and_then(Value::as_str).unwrap_or("").trim();
    let title = if event.get("title").map(|v| !v.is_string() && !v.is_null()).unwrap_or(false) {
        event.get("title").map(|v| v.to_string()).unwrap_or_default()
    } else {
        title.to_string()
    };
    let message = if event.get("message").map(|v| !v.is_string() && !v.is_null()).unwrap_or(false) {
        event.get("message").map(|v| v.to_string()).unwrap_or_default()
    } else {
        message.to_string()
    };
    let title = title.trim().to_string();
    let message = message.trim().to_string();
    if title.is_empty() && message.is_empty() {
        return None;
    }
    let title = if title.is_empty() { APP_NAME.to_string() } else { title };
    Some((title, message))
}

pub fn confirm_id(event: &Value) -> Option<String> {
    let value = event.get("hass_confirm_id")?.as_str()?;
    if value.is_empty() { None } else { Some(value.to_string()) }
}

pub fn entities_from_states(states: &Value) -> Vec<Entity> {
    let Some(list) = states.as_array() else { return Vec::new() };
    let mut out = Vec::new();
    for st in list {
        let Some(eid) = st.get("entity_id").and_then(Value::as_str) else { continue };
        if !eid.contains('.') {
            continue;
        }
        let state = st.get("state").and_then(Value::as_str).unwrap_or("").to_string();
        if action_for(eid, &state).is_none() {
            continue;
        }
        let domain = eid.split_once('.').map(|(d, _)| d.to_string()).unwrap_or_default();
        let name = st
            .get("attributes")
            .and_then(|a| a.get("friendly_name"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(eid)
            .to_string();
        out.push(Entity { entity_id: eid.to_string(), name, domain, state });
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then(a.entity_id.cmp(&b.entity_id)));
    out
}

pub fn state_event(event: &Value) -> Option<Entity> {
    if event.get("event_type").and_then(Value::as_str) != Some("state_changed") {
        return None;
    }
    let new_state = event.get("data")?.get("new_state")?;
    entities_from_states(&Value::Array(vec![new_state.clone()])).into_iter().next()
}

pub fn button_label(entity_id: &str, known: Option<&Entity>, unset: &str) -> String {
    if entity_id.is_empty() {
        return unset.to_string();
    }
    let Some(known) = known else { return entity_id.to_string() };
    if known.state.is_empty() {
        known.name.clone()
    } else {
        format!("{}: {}", known.name, known.state)
    }
}

pub fn button_on(known: Option<&Entity>) -> bool {
    let Some(known) = known else { return false };
    !QUIET.iter().any(|item| *item == known.state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_follow_the_scheme() {
        assert_eq!(ws_url("https://ha.local/").unwrap(), "wss://ha.local/api/websocket");
        assert_eq!(ws_url("http://ha.local").unwrap(), "ws://ha.local/api/websocket");
        assert!(ws_url("ha.local").is_err());
    }

    #[test]
    fn toggle_domains_and_alarm() {
        assert_eq!(action_for("light.kitchen", "on").unwrap().1, "toggle");
        assert_eq!(action_for("script.leave", "").unwrap().1, "turn_on");
        assert_eq!(action_for("lock.door", "locked").unwrap().1, "unlock");
        assert_eq!(action_for("lock.door", "unlocked").unwrap().1, "lock");
        assert_eq!(action_for("alarm_control_panel.home", "armed_away").unwrap().1, "alarm_disarm");
        assert_eq!(action_for("alarm_control_panel.home", "disarmed").unwrap().1, "alarm_arm_away");
        assert_eq!(action_for("climate.hall", "off").unwrap().1, "turn_on");
        assert_eq!(action_for("climate.hall", "heat").unwrap().1, "turn_off");
        assert!(action_for("sensor.temp", "20").is_none());
    }

    #[test]
    fn notification_and_entities() {
        let event = serde_json::json!({"title": "Door", "message": "open", "hass_confirm_id": "abc"});
        assert_eq!(notification_text(&event).unwrap(), ("Door".into(), "open".into()));
        assert_eq!(confirm_id(&event).unwrap(), "abc");
        let states = serde_json::json!([
            {"entity_id": "light.a", "state": "on", "attributes": {"friendly_name": "Lamp"}},
            {"entity_id": "sensor.x", "state": "1", "attributes": {}}
        ]);
        let found = entities_from_states(&states);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "Lamp");
        assert!(button_on(Some(&found[0])));
        assert_eq!(button_label("", None, "Not set"), "Not set");
    }
}
