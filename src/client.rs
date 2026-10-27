// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-or-later

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::sync::mpsc;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

use crate::ha::{self, Entity};

#[derive(Debug)]
pub enum ClientCmd {
    Start(Session),
    Call { entity: String, state: String },
}

#[derive(Clone, Debug)]
pub struct Session {
    pub url: String,
    pub token: String,
    pub webhook_id: String,
    pub device_id: String,
    pub device_name: String,
}

#[derive(Debug)]
pub enum ClientEv {
    Status(u32),
    StatusText(String),
    Registered(String),
    Entities(Vec<Entity>),
    State(Entity),
    Notify(String, String),
    Linked(bool),
}

pub fn spawn(mut cmds: mpsc::UnboundedReceiver<ClientCmd>, out: mpsc::UnboundedSender<ClientEv>) {
    tokio::spawn(async move {
        let mut session: Option<Session> = None;
        let mut calls: Vec<(String, String)> = Vec::new();
        loop {
            if session.is_none() {
                match cmds.recv().await {
                    Some(ClientCmd::Start(next)) => session = Some(next),
                    Some(ClientCmd::Call { .. }) => {
                        let _ = out.send(ClientEv::Status(26));
                    }
                    None => return,
                }
                continue;
            }
            let current = session.clone().unwrap();
            match run_once(current, &mut cmds, &mut calls, &out).await {
                Loop::Stop => return,
                Loop::Retry { session: next, immediate } => {
                    let _ = out.send(ClientEv::Linked(false));
                    session = Some(next);
                    if !immediate {
                        tokio::select! {
                            _ = tokio::time::sleep(Duration::from_secs(5)) => {}
                            cmd = cmds.recv() => match cmd {
                                Some(ClientCmd::Start(next)) => session = Some(next),
                                Some(ClientCmd::Call { .. }) => {}
                                None => return,
                            }
                        }
                    }
                }
            }
        }
    });
}

enum Loop {
    Retry { session: Session, immediate: bool },
    Stop,
}

async fn run_once(
    mut session: Session,
    cmds: &mut mpsc::UnboundedReceiver<ClientCmd>,
    calls: &mut Vec<(String, String)>,
    out: &mpsc::UnboundedSender<ClientEv>,
) -> Loop {
    if session.webhook_id.is_empty() {
        match register(&session).await {
            Ok(webhook) => {
                session.webhook_id = webhook.clone();
                let _ = out.send(ClientEv::Registered(webhook));
            }
            Err(code) => {
                let _ = out.send(match code {
                    27 => ClientEv::Status(27),
                    30 => ClientEv::Status(30),
                    31 => ClientEv::Status(31),
                    28 => ClientEv::Status(28),
                    other => ClientEv::StatusText(crate::i18n::Catalog::load(&crate::i18n::language_dir(), "English").trf(29, &[&other.to_string()])),
                });
                return if code == 27 { Loop::Stop } else { Loop::Retry { session, immediate: false } };
            }
        }
    }
    let url = match ha::ws_url(&session.url) {
        Ok(url) => url,
        Err(_) => {
            let _ = out.send(ClientEv::Status(28));
            return Loop::Stop;
        }
    };
    let Ok((socket, _)) = connect_async(&url).await else {
        let _ = out.send(ClientEv::Status(28));
        return Loop::Retry { session, immediate: false };
    };
    let (mut write, mut read) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();
    let writer = tokio::spawn(async move {
        while let Some(text) = rx.recv().await {
            if write.send(Message::Text(text.into())).await.is_err() {
                break;
            }
        }
    });
    let mut next_id = 0i64;
    let mut push_id: Option<i64> = None;
    let mut state_id: Option<i64> = None;
    let mut pending: Vec<(i64, &'static str)> = Vec::new();
    let mut service_ids: Vec<(i64, String)> = Vec::new();
    let send = |tx: &mpsc::UnboundedSender<String>, id: i64, mut body: Value| {
        if let Some(map) = body.as_object_mut() {
            map.insert("id".into(), json!(id));
        }
        let _ = tx.send(body.to_string());
    };
    loop {
        tokio::select! {
            cmd = cmds.recv() => {
                match cmd {
                    Some(ClientCmd::Start(next)) => {
                        writer.abort();
                        return Loop::Retry { session: next, immediate: true };
                    }
                    Some(ClientCmd::Call { entity, state }) => calls.push((entity, state)),
                    None => {
                        writer.abort();
                        return Loop::Stop;
                    }
                }
            }
            incoming = tokio::time::timeout(Duration::from_secs(90), read.next()) => {
                let Ok(Some(Ok(message))) = incoming else {
                    writer.abort();
                    return Loop::Retry { session, immediate: false };
                };
                let Message::Text(text) = message else { continue };
                let Ok(data) = serde_json::from_str::<Value>(text.as_str()) else { continue };
                let kind = data.get("type").and_then(Value::as_str).unwrap_or("");
                match kind {
                    "auth_required" => {
                        let _ = tx.send(json!({"type": "auth", "access_token": session.token}).to_string());
                    }
                    "auth_invalid" => {
                        let _ = out.send(ClientEv::Status(27));
                        writer.abort();
                        return Loop::Stop;
                    }
                    "auth_ok" => {
                        next_id += 1;
                        push_id = Some(next_id);
                        pending.push((next_id, "push"));
                        send(&tx, next_id, json!({"type": "mobile_app/push_notification_channel", "webhook_id": session.webhook_id, "support_confirm": true}));
                        next_id += 1;
                        pending.push((next_id, "states"));
                        send(&tx, next_id, json!({"type": "get_states"}));
                        next_id += 1;
                        state_id = Some(next_id);
                        send(&tx, next_id, json!({"type": "subscribe_events", "event_type": "state_changed"}));
                        let _ = out.send(ClientEv::Linked(true));
                        let _ = out.send(ClientEv::Status(22));
                    }
                    "ping" => {
                        let id = data.get("id").cloned().unwrap_or(Value::Null);
                        let _ = tx.send(json!({"id": id, "type": "pong"}).to_string());
                    }
                    "event" => {
                        let ident = data.get("id").and_then(Value::as_i64);
                        let Some(event) = data.get("event") else { continue };
                        if ident == push_id {
                            if let Some(cid) = ha::confirm_id(event) {
                                next_id += 1;
                                send(&tx, next_id, json!({"type": "mobile_app/push_notification_confirm", "webhook_id": session.webhook_id, "confirm_id": cid}));
                            }
                            if let Some((title, message)) = ha::notification_text(event) {
                                let _ = out.send(ClientEv::Notify(title, message));
                            }
                        } else if ident == state_id {
                            if let Some(entity) = ha::state_event(event) {
                                let _ = out.send(ClientEv::State(entity));
                            }
                        }
                    }
                    "result" => {
                        let ident = data.get("id").and_then(Value::as_i64);
                        let ok = data.get("success") == Some(&Value::Bool(true));
                        if let Some(id) = ident {
                            if let Some(pos) = pending.iter().position(|(n, _)| *n == id) {
                                let (_, kind) = pending.remove(pos);
                                if kind == "push" && !ok {
                                    session.webhook_id.clear();
                                    writer.abort();
                                    return Loop::Retry { session, immediate: false };
                                }
                                if kind == "states" && ok {
                                    let list = ha::entities_from_states(data.get("result").unwrap_or(&Value::Null));
                                    let _ = out.send(ClientEv::Entities(list));
                                }
                            }
                            if let Some(pos) = service_ids.iter().position(|(n, _)| *n == id) {
                                let (_, entity) = service_ids.remove(pos);
                                if !ok {
                                    let message = data.get("error").and_then(|e| e.get("message")).and_then(Value::as_str).unwrap_or("").to_string();
                                    if message.is_empty() {
                                        let _ = out.send(ClientEv::Status(33));
                                    } else {
                                        let _ = out.send(ClientEv::StatusText(message));
                                    }
                                }
                                let _ = entity;
                            }
                        }
                    }
                    _ => {}
                }
                while let Some((entity, state)) = calls.pop() {
                    if push_id.is_none() {
                        let _ = out.send(ClientEv::Status(26));
                        continue;
                    }
                    let Some((domain, service)) = ha::action_for(&entity, &state) else {
                        let _ = out.send(ClientEv::Status(33));
                        continue;
                    };
                    next_id += 1;
                    service_ids.push((next_id, entity.clone()));
                    send(&tx, next_id, json!({"type": "call_service", "domain": domain, "service": service, "target": {"entity_id": entity}}));
                }
            }
        }
    }
}

async fn register(session: &Session) -> Result<String, u16> {
    let url = ha::http_url(&session.url, "/api/mobile_app/registrations");
    if ha::ws_url(&session.url).is_err() {
        return Err(28);
    }
    let client = reqwest::Client::builder().use_rustls_tls().build().map_err(|_| 28u16)?;
    let os_version = std::fs::read_to_string("/proc/sys/kernel/osrelease").unwrap_or_else(|_| "linux".into());
    let body = ha::registration_body(&session.device_id, &session.device_name, env!("CARGO_PKG_VERSION"), os_version.trim());
    let response = client
        .post(url)
        .bearer_auth(&session.token)
        .json(&body)
        .send()
        .await
        .map_err(|_| 28u16)?;
    let code = response.status().as_u16();
    if code == 401 || code == 403 {
        return Err(27);
    }
    if code != 200 && code != 201 {
        return Err(code);
    }
    let data: Value = response.json().await.map_err(|_| 30u16)?;
    data.get("webhook_id").and_then(Value::as_str).filter(|s| !s.is_empty()).map(|s| s.to_string()).ok_or(31)
}
