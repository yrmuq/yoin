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
    Notify { title: String, message: String, image: Option<String> },
    Shot { id: String, path: String },
    Connection(u32),
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
                Loop::Wait => {
                    session = None;
                    calls.clear();
                }
                Loop::Retry { session: next, immediate } => {
                    session = Some(next);
                    if !immediate {
                        tokio::select! {
                            _ = tokio::time::sleep(Duration::from_secs(5)) => {}
                            cmd = cmds.recv() => match cmd {
                                Some(ClientCmd::Start(next)) => {
                                    calls.clear();
                                    session = Some(next);
                                }
                                Some(ClientCmd::Call { entity, state }) => calls.push((entity, state)),
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
    Wait,
    Stop,
}

async fn run_once(
    mut session: Session,
    cmds: &mut mpsc::UnboundedReceiver<ClientCmd>,
    calls: &mut Vec<(String, String)>,
    out: &mpsc::UnboundedSender<ClientEv>,
) -> Loop {
    let _ = out.send(ClientEv::Connection(21));
    if session.webhook_id.is_empty() {
        match register(&session).await {
            Ok(webhook) => {
                session.webhook_id = webhook.clone();
                let _ = out.send(ClientEv::Registered(webhook));
            }
            Err(code) => {
                let _ = out.send(match code {
                    27 => ClientEv::Connection(27),
                    30 => ClientEv::Connection(30),
                    31 => ClientEv::Connection(31),
                    28 => ClientEv::Connection(28),
                    other => {
                        let _ = out.send(ClientEv::Connection(32));
                        ClientEv::StatusText(crate::i18n::Catalog::load(&crate::i18n::language_dir(), "English").trf(29, &[&other.to_string()]))
                    },
                });
                return if code == 27 { Loop::Wait } else { Loop::Retry { session, immediate: false } };
            }
        }
    }
    let url = match ha::ws_url(&session.url) {
        Ok(url) => url,
        Err(_) => {
            let _ = out.send(ClientEv::Connection(28));
            return Loop::Wait;
        }
    };
    let Ok(Ok((socket, _))) = tokio::time::timeout(Duration::from_secs(15), connect_async(&url)).await else {
        let _ = out.send(ClientEv::Connection(28));
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
    let mut ready = false;
    let mut heartbeat = tokio::time::interval(Duration::from_secs(30));
    heartbeat.tick().await;
    let mut last_received = tokio::time::Instant::now();
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
                        calls.clear();
                        writer.abort();
                        return Loop::Retry { session: next, immediate: true };
                    }
                    Some(ClientCmd::Call { entity, state }) => {
                        if !ready {
                            calls.push((entity, state));
                        } else {
                            send_service(&tx, &mut next_id, &mut service_ids, out, &entity, &state);
                        }
                    }
                    None => {
                        writer.abort();
                        return Loop::Stop;
                    }
                }
            }
            _ = heartbeat.tick() => {
                if !ready {
                    let _ = out.send(ClientEv::Connection(32));
                    writer.abort();
                    return Loop::Retry { session, immediate: false };
                }
                if last_received.elapsed() >= Duration::from_secs(90) {
                    let _ = out.send(ClientEv::Connection(24));
                    writer.abort();
                    return Loop::Retry { session, immediate: false };
                }
                next_id += 1;
                send(&tx, next_id, json!({"type": "ping"}));
            }
            incoming = read.next() => {
                let Some(Ok(message)) = incoming else {
                    let _ = out.send(ClientEv::Connection(24));
                    writer.abort();
                    return Loop::Retry { session, immediate: false };
                };
                last_received = tokio::time::Instant::now();
                if matches!(message, Message::Close(_)) {
                    let _ = out.send(ClientEv::Connection(24));
                    writer.abort();
                    return Loop::Retry { session, immediate: false };
                }
                let Message::Text(text) = message else { continue };
                let Ok(data) = serde_json::from_str::<Value>(text.as_str()) else { continue };
                let kind = data.get("type").and_then(Value::as_str).unwrap_or("");
                match kind {
                    "auth_required" => {
                        let _ = tx.send(json!({"type": "auth", "access_token": session.token}).to_string());
                    }
                    "auth_invalid" => {
                        let _ = out.send(ClientEv::Connection(27));
                        writer.abort();
                        return Loop::Wait;
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
                        pending.push((next_id, "events"));
                        send(&tx, next_id, json!({"type": "subscribe_events", "event_type": "state_changed"}));
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
                                let image = ha::notification_image(event);
                                let _ = out.send(ClientEv::Notify { title, message, image });
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
                                if !ok {
                                    if kind == "push" { session.webhook_id.clear(); }
                                    let _ = out.send(ClientEv::Connection(32));
                                    writer.abort();
                                    return Loop::Retry { session, immediate: false };
                                }
                                if kind == "states" && ok {
                                    let list = ha::entities_from_states(data.get("result").unwrap_or(&Value::Null));
                                    let _ = out.send(ClientEv::Entities(list));
                                }
                                if pending.is_empty() && !ready {
                                    ready = true;
                                    let _ = out.send(ClientEv::Connection(22));
                                    for (entity, state) in calls.drain(..) {
                                        send_service(&tx, &mut next_id, &mut service_ids, out, &entity, &state);
                                    }
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
            }
        }
    }
}

fn send_service(
    tx: &tokio::sync::mpsc::UnboundedSender<String>,
    next_id: &mut i64,
    service_ids: &mut Vec<(i64, String)>,
    out: &tokio::sync::mpsc::UnboundedSender<ClientEv>,
    entity: &str,
    state: &str,
) {
    let Some((domain, service)) = ha::action_for(entity, state) else {
        let _ = out.send(ClientEv::Status(33));
        return;
    };
    *next_id += 1;
    service_ids.push((*next_id, entity.to_string()));
    let body = json!({
        "id": *next_id,
        "type": "call_service",
        "domain": domain,
        "service": service,
        "target": {"entity_id": entity},
    });
    let _ = tx.send(body.to_string());
}

async fn register(session: &Session) -> Result<String, u16> {
    let url = ha::http_url(&session.url, "/api/mobile_app/registrations");
    if ha::ws_url(&session.url).is_err() {
        return Err(28);
    }
    let client = reqwest::Client::builder().use_rustls_tls().timeout(Duration::from_secs(15)).build().map_err(|_| 28u16)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn connection_waits_for_subscriptions_and_reports_failure_or_disconnect() {
        tokio::time::timeout(Duration::from_secs(5), async {
            for accepted in [false, true] {
                let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
                let url = format!("http://{}", listener.local_addr().unwrap());
                let (release, wait) = tokio::sync::oneshot::channel();
                let server = tokio::spawn(async move {
                    let (stream, _) = listener.accept().await.unwrap();
                    let mut socket = tokio_tungstenite::accept_async(stream).await.unwrap();
                    socket.send(Message::Text(json!({"type": "auth_ok"}).to_string().into())).await.unwrap();
                    for index in 0..3 {
                        let request = socket.next().await.unwrap().unwrap();
                        let request: Value = serde_json::from_str(request.to_text().unwrap()).unwrap();
                        if index == 2 {
                            wait.await.unwrap();
                            socket.send(Message::Text(json!({"id": request["id"], "type": "result", "success": accepted}).to_string().into())).await.unwrap();
                            if accepted { socket.close(None).await.unwrap(); }
                            break;
                        }
                        socket.send(Message::Text(json!({"id": request["id"], "type": "result", "success": true, "result": []}).to_string().into())).await.unwrap();
                    }
                });
                let (_tx, mut rx) = mpsc::unbounded_channel();
                let (out, mut events) = mpsc::unbounded_channel();
                let worker = tokio::spawn(async move {
                    let session = Session { url, token: "test".into(), webhook_id: "registered".into(), device_id: "test".into(), device_name: "test".into() };
                    run_once(session, &mut rx, &mut Vec::new(), &out).await;
                });
                assert!(matches!(events.recv().await.unwrap(), ClientEv::Connection(21)));
                assert!(matches!(events.recv().await.unwrap(), ClientEv::Entities(_)));
                assert!(tokio::time::timeout(Duration::from_millis(50), events.recv()).await.is_err(), "reported connected before subscription confirmation");
                release.send(()).unwrap();
                let expected = if accepted { 22 } else { 32 };
                assert!(matches!(events.recv().await.unwrap(), ClientEv::Connection(id) if id == expected));
                if accepted { assert!(matches!(events.recv().await.unwrap(), ClientEv::Connection(24))); }
                worker.await.unwrap();
                server.await.unwrap();
            }
        }).await.expect("connection status did not settle");
    }

    #[tokio::test]
    async fn rejected_registration_keeps_accepting_settings() {
        use std::io::{Read, Write};

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut request = [0; 4096];
            stream.read(&mut request).unwrap();
            stream.write_all(b"HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
        });
        tokio::time::timeout(Duration::from_secs(5), async {
            let (tx, rx) = mpsc::unbounded_channel();
            let (out, mut events) = mpsc::unbounded_channel();
            spawn(rx, out);
            let mut session = Session { url, token: "invalid".into(), webhook_id: String::new(), device_id: "test".into(), device_name: "test".into() };
            tx.send(ClientCmd::Start(session.clone())).unwrap();
            while !matches!(events.recv().await.unwrap(), ClientEv::Connection(27)) {}
            session.url = "invalid-address".into();
            session.webhook_id = "registered".into();
            tx.send(ClientCmd::Start(session)).unwrap();
            while !matches!(events.recv().await.unwrap(), ClientEv::Connection(28)) {}
        }).await.expect("registration rejection stopped the client");
        server.join().unwrap();
    }

    #[tokio::test]
    async fn corrected_credentials_reconnect_after_rejection() {
        tokio::time::timeout(Duration::from_secs(5), async {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let url = format!("http://{}", listener.local_addr().unwrap());
            let server = tokio::spawn(async move {
                for accepted in [false, true] {
                    let (stream, _) = listener.accept().await.unwrap();
                    let mut socket = tokio_tungstenite::accept_async(stream).await.unwrap();
                    socket.send(Message::Text(json!({"type": "auth_required"}).to_string().into())).await.unwrap();
                    let auth = socket.next().await.unwrap().unwrap();
                    let auth: Value = serde_json::from_str(auth.to_text().unwrap()).unwrap();
                    assert_eq!(auth["access_token"], if accepted { "corrected" } else { "invalid" });
                    let kind = if accepted { "auth_ok" } else { "auth_invalid" };
                    socket.send(Message::Text(json!({"type": kind}).to_string().into())).await.unwrap();
                    if accepted {
                        for _ in 0..3 {
                            let request = socket.next().await.unwrap().unwrap();
                            let request: Value = serde_json::from_str(request.to_text().unwrap()).unwrap();
                            socket.send(Message::Text(json!({"id": request["id"], "type": "result", "success": true, "result": []}).to_string().into())).await.unwrap();
                        }
                    }
                }
            });
            let (tx, rx) = mpsc::unbounded_channel();
            let (out, mut events) = mpsc::unbounded_channel();
            spawn(rx, out);
            let mut session = Session { url, token: "invalid".into(), webhook_id: "registered".into(), device_id: "test".into(), device_name: "test".into() };
            tx.send(ClientCmd::Start(session.clone())).unwrap();
            loop {
                if matches!(events.recv().await.unwrap(), ClientEv::Connection(27)) {
                    break;
                }
            }
            session.token = "corrected".into();
            tx.send(ClientCmd::Start(session)).unwrap();
            loop {
                if matches!(events.recv().await.unwrap(), ClientEv::Connection(22)) {
                    break;
                }
            }
            server.await.unwrap();
            drop(tx);
        }).await.expect("client stopped accepting connection settings");
    }
}
