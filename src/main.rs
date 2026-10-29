// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-or-later

mod client;
mod config;
mod ha;
mod icons;
mod i18n;
mod notify;
mod place;
mod theme;
mod tray;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::fs::File;
use std::os::fd::AsRawFd;
use std::rc::Rc;
use std::time::{Duration, Instant};

use std::sync::{Arc, Mutex};

use slint::winit_030::{winit, EventResult, WinitWindowAccessor};
use slint::{ComponentHandle, ModelRc, VecModel};

use crate::client::{ClientCmd, ClientEv, Session};
use crate::config::Config;
use crate::ha::Entity;
use crate::i18n::Catalog;
use crate::tray::{TrayCmd, TrayView, YoinTray};

slint::include_modules!();

type TrayHandle = Arc<ksni::Handle<YoinTray>>;

struct App {
    cfg: Config,
    catalog: Catalog,
    entities: HashMap<String, Entity>,
    names: Vec<Entity>,
    dark: bool,
    popup_open: bool,
    placer_ok: bool,
    place_warned: bool,
    status: String,
}

struct Shell {
    settings: Option<Settings>,
    popup: Option<Popup>,
}

struct Ctx {
    app: Rc<RefCell<App>>,
    shell: Rc<RefCell<Shell>>,
    client_tx: tokio::sync::mpsc::UnboundedSender<ClientCmd>,
    view: Arc<Mutex<TrayView>>,
    tray: TrayHandle,
    rt: tokio::runtime::Handle,
}

fn main() {
    let _lock = match single_instance() {
        Some(file) => file,
        None => return,
    };
    if let Err(err) = slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("femtovg".into())
        .with_winit_window_attributes_hook(|attributes| attributes.with_transparent(true))
        .select()
    {
        eprintln!("yoin: graphics: {err}");
        return;
    }
    // Wayland reports this as the window class. KWin uses it to keep the
    // process out of the taskbar before the window is mapped.
    let _ = slint::set_xdg_app_id("yoin");
    let mut cfg = config::load_config(&config::config_path());
    let folder = i18n::language_dir();
    if cfg.language.is_empty() {
        cfg.language = i18n::match_language(&folder, &i18n::system_languages());
        let _ = config::save_config(&cfg, &config::config_path());
    }
    if cfg.device_id.is_empty() {
        cfg.device_id = uuid::Uuid::new_v4().simple().to_string();
    }
    if cfg.device_name.is_empty() {
        cfg.device_name = config::hostname();
    }
    let catalog = Catalog::load(&folder, &cfg.language);
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let (tray_tx, mut tray_rx) = tokio::sync::mpsc::unbounded_channel();
    let (client_tx, client_rx) = tokio::sync::mpsc::unbounded_channel();
    let (ev_tx, mut ev_rx) = tokio::sync::mpsc::unbounded_channel();
    let view = std::sync::Arc::new(std::sync::Mutex::new(TrayView {
        settings: catalog.tr(1),
        quit: catalog.tr(2),
        icon: Vec::new(),
    }));
    let tray = YoinTray { view: view.clone(), tx: tray_tx };
    let rt_handle = rt.handle().clone();
    let (tray_handle, placer_ok) = rt.block_on(async {
        client::spawn(client_rx, ev_tx);
        let placer_ok = place::install().await;
        let handle = ksni::TrayMethods::assume_sni_available(tray, true)
            .spawn()
            .await
            .expect("tray");
        (Arc::new(handle), placer_ok)
    });
    let ctx = Rc::new(Ctx {
        app: Rc::new(RefCell::new(App {
            cfg,
            catalog,
            entities: HashMap::new(),
            names: Vec::new(),
            dark: false,
            popup_open: false,
            placer_ok,
            place_warned: false,
            status: String::new(),
        })),
        shell: Rc::new(RefCell::new(Shell { settings: None, popup: None })),
        client_tx: client_tx.clone(),
        view: view.clone(),
        tray: tray_handle.clone(),
        rt: rt_handle.clone(),
    });
    paint_theme(&ctx);
    if ctx.app.borrow().cfg.autostart {
        let _ = config::apply_autostart(true, &config::autostart_path());
    }
    if ctx.app.borrow().cfg.url.is_empty() || ctx.app.borrow().cfg.token.is_empty() {
        let settings = ensure_settings(&ctx);
        settings.show().ok();
    } else {
        start_client(&ctx.app, &ctx.client_tx);
    }
    let ctx_timer = ctx.clone();
    let last_toggle = Cell::new(None::<Instant>);
    let mut sharpen_left = 0u8;
    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::Repeated, Duration::from_millis(80), move || {
        let mut toggles = 0u32;
        while let Ok(cmd) = tray_rx.try_recv() {
            match cmd {
                TrayCmd::Settings => {
                    let settings = ensure_settings(&ctx_timer);
                    settings.show().ok();
                }
                TrayCmd::Toggle => toggles += 1,
            }
        }
        if toggles > 0 {
            let now = Instant::now();
            // Plasma can deliver one click as several Activate calls. An even
            // count must still open the window, not cancel itself.
            let allow = last_toggle.get().is_none_or(|at| now.duration_since(at) >= Duration::from_millis(300));
            if allow {
                last_toggle.set(Some(now));
                if toggle_popup(&ctx_timer) {
                    sharpen_left = 8;
                }
            }
        }
        while let Ok(ev) = ev_rx.try_recv() {
            match ev {
                ClientEv::Status(id) => note_status(&ctx_timer, ctx_timer.app.borrow().catalog.tr(id)),
                ClientEv::StatusText(text) => note_status(&ctx_timer, text),
                ClientEv::Registered(webhook) => {
                    ctx_timer.app.borrow_mut().cfg.webhook_id = webhook;
                    let _ = config::save_config(&ctx_timer.app.borrow().cfg, &config::config_path());
                }
                ClientEv::Entities(list) => {
                    ctx_timer.app.borrow_mut().entities = list.iter().map(|e| (e.entity_id.clone(), e.clone())).collect();
                    ctx_timer.app.borrow_mut().names = list;
                    if let Some(settings) = settings_of(&ctx_timer.shell) {
                        fill_rows(&ctx_timer.app, &settings);
                    }
                    if let Some(popup) = popup_of(&ctx_timer.shell) {
                        refresh_popup(&ctx_timer.app, &popup);
                    }
                }
                ClientEv::State(entity) => {
                    ctx_timer.app.borrow_mut().entities.insert(entity.entity_id.clone(), entity.clone());
                    if let Some(slot) = ctx_timer.app.borrow_mut().names.iter_mut().find(|item| item.entity_id == entity.entity_id) {
                        *slot = entity;
                    }
                    if let Some(popup) = popup_of(&ctx_timer.shell) {
                        refresh_popup(&ctx_timer.app, &popup);
                    }
                }
                ClientEv::Notify(title, message) => {
                    let icon = icons::asset_path("icon.png");
                    let icon = icon.display().to_string();
                    ctx_timer.rt.spawn(async move {
                        notify::announce(&title, &message, &icon).await;
                    });
                }
                ClientEv::Linked(up) => {
                    let id = if up { 22 } else { 24 };
                    let text = ctx_timer.app.borrow().catalog.tr(id);
                    note_status(&ctx_timer, text);
                }
            }
        }
        if ctx_timer.app.borrow().cfg.theme == "system" {
            let dark = theme::system_dark();
            if dark != ctx_timer.app.borrow().dark {
                paint_theme(&ctx_timer);
            }
        }
        if sharpen_left > 0 {
            if ctx_timer.app.borrow().popup_open {
                if let Some(popup) = popup_of(&ctx_timer.shell) {
                    sharpen_popup(&popup);
                }
            }
            sharpen_left -= 1;
        }
    });
    // ksni is not a Slint tray icon, so an idle popup must not end the process.
    slint::run_event_loop_until_quit().ok();
}

fn single_instance() -> Option<File> {
    let dir = config::config_dir();
    let _ = std::fs::create_dir_all(&dir);
    let file = File::create(dir.join("yoin.lock")).ok()?;
    let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if rc == 0 { Some(file) } else { None }
}

fn start_client(app: &Rc<RefCell<App>>, tx: &tokio::sync::mpsc::UnboundedSender<ClientCmd>) {
    let cfg = app.borrow().cfg.clone();
    if cfg.url.is_empty() || cfg.token.is_empty() {
        return;
    }
    let _ = tx.send(ClientCmd::Start(Session {
        url: cfg.url,
        token: cfg.token,
        webhook_id: cfg.webhook_id,
        device_id: cfg.device_id,
        device_name: cfg.device_name,
    }));
}

fn settings_of(shell: &RefCell<Shell>) -> Option<Settings> {
    shell.borrow().settings.as_ref().map(Settings::clone_strong)
}

fn popup_of(shell: &RefCell<Shell>) -> Option<Popup> {
    shell.borrow().popup.as_ref().map(Popup::clone_strong)
}

fn note_status(ctx: &Ctx, text: String) {
    ctx.app.borrow_mut().status = text.clone();
    if let Some(settings) = settings_of(&ctx.shell) {
        settings.set_status(text.into());
    }
}

fn paint_theme(ctx: &Ctx) {
    let settings = settings_of(&ctx.shell);
    let popup = popup_of(&ctx.shell);
    apply_theme(&ctx.app, settings.as_ref(), popup.as_ref(), &ctx.view, &ctx.tray, &ctx.rt);
}

fn ensure_settings(ctx: &Ctx) -> Settings {
    if let Some(settings) = settings_of(&ctx.shell) {
        return settings;
    }
    let settings = Settings::new().unwrap();
    let _ = settings.window().on_close_requested(|| slint::CloseRequestResponse::HideWindow);
    ctx.shell.borrow_mut().settings = Some(settings.clone_strong());
    paint_theme(ctx);
    fill_text(&ctx.app, &settings);
    fill_rows(&ctx.app, &settings);
    if !ctx.app.borrow().status.is_empty() {
        settings.set_status(ctx.app.borrow().status.clone().into());
    }
    wire_settings(ctx, &settings);
    settings
}

fn ensure_popup(ctx: &Ctx) -> Popup {
    if let Some(popup) = popup_of(&ctx.shell) {
        return popup;
    }
    let popup = Popup::new().unwrap();
    let _ = popup.window().on_close_requested(|| slint::CloseRequestResponse::HideWindow);
    let weak = popup.as_weak();
    let _ = popup.window().on_winit_window_event(move |_window, event| {
        if matches!(event, winit::event::WindowEvent::ScaleFactorChanged { .. }) {
            if let Some(popup) = weak.upgrade() {
                let _ = sharpen_popup(&popup);
            }
        }
        EventResult::Propagate
    });
    ctx.shell.borrow_mut().popup = Some(popup.clone_strong());
    paint_theme(ctx);
    refresh_popup(&ctx.app, &popup);
    wire_popup(ctx, &popup);
    popup
}

fn ctx_from(ctx: &Ctx) -> Ctx {
    Ctx {
        app: ctx.app.clone(),
        shell: ctx.shell.clone(),
        client_tx: ctx.client_tx.clone(),
        view: ctx.view.clone(),
        tray: ctx.tray.clone(),
        rt: ctx.rt.clone(),
    }
}

fn sharpen_popup(popup: &Popup) -> bool {
    let width = popup.get_layout_width();
    let height = popup.get_layout_height();
    if width < 2.0 || height < 2.0 {
        return false;
    }
    let pending = popup.window().with_winit_window(|window| {
        let scale = window.scale_factor();
        if !scale.is_finite() || scale <= 0.0 {
            return true;
        }
        let snapped = (theme::system_font().size_px * scale as f32).round().max(1.0) / scale as f32;
        if (popup.get_ui_font_size() - snapped).abs() > 0.05 {
            popup.set_ui_font_size(snapped);
        }
        let physical = window.inner_size();
        let want_w = (width as f64 * scale).round() as u32;
        let want_h = (height as f64 * scale).round() as u32;
        // The Wayland window is often created before the scale arrives, so the
        // buffer stays at 1x and the compositor stretches it. Asking again in
        // logical pixels reallocates the buffer at the real scale.
        if physical.width.abs_diff(want_w) > 1 || physical.height.abs_diff(want_h) > 1 {
            let _ = window.request_inner_size(winit::dpi::LogicalSize::new(width as f64, height as f64));
            return true;
        }
        false
    }).unwrap_or(true);
    popup.window().request_redraw();
    pending
}

fn toggle_popup(ctx: &Ctx) -> bool {
    let popup = ensure_popup(ctx);
    if ctx.app.borrow().popup_open {
        popup.hide().ok();
        ctx.app.borrow_mut().popup_open = false;
        return false;
    }
    refresh_popup(&ctx.app, &popup);
    popup.show().ok();
    let _ = popup.window().with_winit_window(|window| window.set_title("yoin-buttons"));
    let _ = sharpen_popup(&popup);
    ctx.app.borrow_mut().popup_open = true;
    if !ctx.app.borrow().placer_ok && !ctx.app.borrow().place_warned {
        ctx.app.borrow_mut().place_warned = true;
        note_status(ctx, ctx.app.borrow().catalog.tr(23));
    }
    true
}

fn refresh_popup(app: &Rc<RefCell<App>>, popup: &Popup) {
    let app = app.borrow();
    let unset = app.catalog.tr(12);
    let faces: Vec<Face> = app.cfg.buttons.iter().map(|id| {
        let known = app.entities.get(id);
        Face {
            text: ha::button_label(id, known, &unset).into(),
            on: ha::button_on(known),
        }
    }).collect();
    popup.set_faces(ModelRc::new(VecModel::from(faces)));
}

fn fill_text(app: &Rc<RefCell<App>>, settings: &Settings) {
    let app_ref = app.borrow();
    let cat = &app_ref.catalog;
    settings.set_t_connection(cat.tr(10).into());
    settings.set_t_buttons(cat.tr(11).into());
    settings.set_t_appearance(cat.tr(18).into());
    settings.set_t_address(cat.tr(3).into());
    settings.set_t_token(cat.tr(4).into());
    settings.set_t_token_hint(cat.tr(25).into());
    settings.set_t_device(cat.tr(5).into());
    settings.set_t_device_hint(cat.tr(6).into());
    settings.set_t_autostart(cat.tr(7).into());
    settings.set_t_connect(cat.tr(8).into());
    settings.set_t_save(cat.tr(9).into());
    settings.set_t_language(cat.tr(13).into());
    settings.set_t_theme(cat.tr(14).into());
    settings.set_version(env!("CARGO_PKG_VERSION").into());
    settings.set_address(app_ref.cfg.url.clone().into());
    settings.set_token(app_ref.cfg.token.clone().into());
    settings.set_device(app_ref.cfg.device_name.clone().into());
    settings.set_autostart(app_ref.cfg.autostart);
    let names = i18n::languages(&i18n::language_dir());
    let index = names.iter().position(|n| n == &app_ref.cfg.language).unwrap_or(0) as i32;
    settings.set_language_names(ModelRc::new(VecModel::from(names.into_iter().map(Into::into).collect::<Vec<slint::SharedString>>())));
    settings.set_language_index(index);
    settings.set_theme_names(ModelRc::new(VecModel::from(vec![
        cat.tr(15).into(),
        cat.tr(16).into(),
        cat.tr(17).into(),
    ])));
    let theme_index = match app_ref.cfg.theme.as_str() {
        "light" => 1,
        "dark" => 2,
        _ => 0,
    };
    settings.set_theme_index(theme_index);
}

fn fill_rows(app: &Rc<RefCell<App>>, settings: &Settings) {
    let app_ref = app.borrow();
    let unset = app_ref.catalog.tr(12);
    let mut labels = vec![unset.into()];
    labels.extend(app_ref.names.iter().map(|e| e.name.clone().into()));
    settings.set_entity_names(ModelRc::new(VecModel::from(labels)));
    let rows: Vec<Choice> = app_ref.cfg.buttons.iter().map(|id| {
        let index = if id.is_empty() {
            0
        } else {
            app_ref.names.iter().position(|e| e.entity_id == *id).map(|i| i as i32 + 1).unwrap_or(0)
        };
        Choice { index }
    }).collect();
    settings.set_rows(ModelRc::new(VecModel::from(rows)));
}

fn apply_theme(
    app: &Rc<RefCell<App>>,
    settings: Option<&Settings>,
    popup: Option<&Popup>,
    view: &Arc<Mutex<TrayView>>,
    tray: &TrayHandle,
    rt: &tokio::runtime::Handle,
) {
    let dark = theme::resolved(&app.borrow().cfg.theme, theme::system_dark()) == "dark";
    app.borrow_mut().dark = dark;
    let font = theme::system_font();
    let family = slint::SharedString::from(font.family);
    if let Some(settings) = settings {
        settings.set_dark(dark);
        settings.set_ui_font_family(family.clone());
        settings.set_ui_font_size(font.size_px);
        settings.set_ui_font_weight(font.weight);
    }
    if let Some(popup) = popup {
        popup.set_dark(dark);
        popup.set_ui_font_family(family);
        popup.set_ui_font_size(font.size_px);
        popup.set_ui_font_weight(font.weight);
    }
    let file = if dark { "tray-light.png" } else { "tray-dark.png" };
    let mut path = icons::asset_path(file);
    if !path.is_file() {
        path = icons::asset_path("icon.png");
    }
    if let Some(icon) = icons::load_tray_icon(&path) {
        if let Ok(mut view) = view.lock() {
            view.icon = vec![icon];
        }
        rt.block_on(tray.update(|_| ()));
    }
}

fn wire_settings(ctx: &Ctx, settings: &Settings) {
    let app_c = ctx.app.clone();
    let tx = ctx.client_tx.clone();
    let settings_c = settings.as_weak();
    settings.on_connect(move || {
        if let Some(settings) = settings_c.upgrade() {
            store(&app_c, &settings, &tx, false);
        }
    });
    let app_s = ctx.app.clone();
    let tx = ctx.client_tx.clone();
    let settings_s = settings.as_weak();
    settings.on_save(move || {
        if let Some(settings) = settings_s.upgrade() {
            if store(&app_s, &settings, &tx, true) {
                settings.hide().ok();
            }
        }
    });
    let app_a = ctx.app.clone();
    let shell_a = ctx.shell.clone();
    let settings_a = settings.as_weak();
    settings.on_add(move || {
        if app_a.borrow().cfg.buttons.len() >= config::MAX_BUTTONS {
            return;
        }
        app_a.borrow_mut().cfg.buttons.push(String::new());
        let _ = config::save_config(&app_a.borrow().cfg, &config::config_path());
        if let Some(settings) = settings_a.upgrade() {
            fill_rows(&app_a, &settings);
        }
        if let Some(popup) = popup_of(&shell_a) {
            refresh_popup(&app_a, &popup);
        }
    });
    bind_move(&ctx.app, &ctx.shell, settings, true);
    bind_move(&ctx.app, &ctx.shell, settings, false);
    let app_r = ctx.app.clone();
    let shell_r = ctx.shell.clone();
    let settings_r = settings.as_weak();
    settings.on_remove(move |index| {
        let mut cfg_app = app_r.borrow_mut();
        if cfg_app.cfg.buttons.len() <= 1 {
            return;
        }
        let index = index as usize;
        if index < cfg_app.cfg.buttons.len() {
            cfg_app.cfg.buttons.remove(index);
        }
        drop(cfg_app);
        let _ = config::save_config(&app_r.borrow().cfg, &config::config_path());
        if let Some(settings) = settings_r.upgrade() {
            fill_rows(&app_r, &settings);
        }
        if let Some(popup) = popup_of(&shell_r) {
            refresh_popup(&app_r, &popup);
        }
    });
    let app_k = ctx.app.clone();
    let shell_k = ctx.shell.clone();
    let settings_k = settings.as_weak();
    settings.on_pick(move |row, index| {
        let mut borrowed = app_k.borrow_mut();
        let row = row as usize;
        if row >= borrowed.cfg.buttons.len() {
            return;
        }
        let id = if index <= 0 {
            String::new()
        } else {
            borrowed.names.get(index as usize - 1).map(|e| e.entity_id.clone()).unwrap_or_default()
        };
        if borrowed.cfg.buttons[row] == id {
            return;
        }
        borrowed.cfg.buttons[row] = id;
        drop(borrowed);
        let _ = config::save_config(&app_k.borrow().cfg, &config::config_path());
        if let Some(popup) = popup_of(&shell_k) {
            refresh_popup(&app_k, &popup);
        }
        if let Some(settings) = settings_k.upgrade() {
            fill_rows(&app_k, &settings);
        }
    });
    let app_l = ctx.app.clone();
    let settings_l = settings.as_weak();
    let view_l = ctx.view.clone();
    let tray_l = ctx.tray.clone();
    let rt_l = ctx.rt.clone();
    settings.on_language(move |index| {
        let names = i18n::languages(&i18n::language_dir());
        let Some(name) = names.get(index as usize) else { return };
        if app_l.borrow().cfg.language == *name {
            return;
        }
        app_l.borrow_mut().cfg.language = name.clone();
        app_l.borrow_mut().catalog = Catalog::load(&i18n::language_dir(), name);
        let _ = config::save_config(&app_l.borrow().cfg, &config::config_path());
        if let Ok(mut view) = view_l.lock() {
            view.settings = app_l.borrow().catalog.tr(1);
            view.quit = app_l.borrow().catalog.tr(2);
        }
        rt_l.block_on(tray_l.update(|_| ()));
        if let Some(settings) = settings_l.upgrade() {
            fill_text(&app_l, &settings);
            fill_rows(&app_l, &settings);
        }
    });
    let ctx_t = ctx_from(ctx);
    settings.on_theme(move |index| {
        let theme = match index {
            1 => "light",
            2 => "dark",
            _ => "system",
        };
        if ctx_t.app.borrow().cfg.theme == theme {
            return;
        }
        ctx_t.app.borrow_mut().cfg.theme = theme.into();
        let _ = config::save_config(&ctx_t.app.borrow().cfg, &config::config_path());
        paint_theme(&ctx_t);
    });
}

fn wire_popup(ctx: &Ctx, popup: &Popup) {
    let app_p = ctx.app.clone();
    let tx = ctx.client_tx.clone();
    let ctx_p = ctx_from(ctx);
    popup.on_pressed(move |index| {
        let entity = app_p.borrow().cfg.buttons.get(index as usize).cloned().unwrap_or_default();
        if entity.is_empty() {
            let settings = ensure_settings(&ctx_p);
            settings.show().ok();
            return;
        }
        let state = app_p.borrow().entities.get(&entity).map(|e| e.state.clone()).unwrap_or_default();
        let _ = tx.send(ClientCmd::Call { entity, state });
    });
}

fn bind_move(app: &Rc<RefCell<App>>, shell: &Rc<RefCell<Shell>>, settings: &Settings, up: bool) {
    let app = app.clone();
    let shell = shell.clone();
    let settings_w = settings.as_weak();
    let go = move |index: i32| {
        let index = index as usize;
        let mut borrowed = app.borrow_mut();
        let len = borrowed.cfg.buttons.len();
        if up {
            if index == 0 || index >= len {
                return;
            }
            borrowed.cfg.buttons.swap(index, index - 1);
        } else if index + 1 >= len {
            return;
        } else {
            borrowed.cfg.buttons.swap(index, index + 1);
        }
        drop(borrowed);
        let _ = config::save_config(&app.borrow().cfg, &config::config_path());
        if let Some(settings) = settings_w.upgrade() {
            fill_rows(&app, &settings);
        }
        if let Some(popup) = popup_of(&shell) {
            refresh_popup(&app, &popup);
        }
    };
    if up {
        settings.on_up(go);
    } else {
        settings.on_down(go);
    }
}

fn store(app: &Rc<RefCell<App>>, settings: &Settings, tx: &tokio::sync::mpsc::UnboundedSender<ClientCmd>, close: bool) -> bool {
    let mut cfg = app.borrow().cfg.clone();
    cfg.url = settings.get_address().to_string();
    cfg.token = settings.get_token().to_string();
    cfg.device_name = settings.get_device().trim().to_string();
    if cfg.device_name.is_empty() {
        cfg.device_name = config::hostname();
        settings.set_device(cfg.device_name.clone().into());
    }
    cfg.autostart = settings.get_autostart();
    if cfg.url.is_empty() || cfg.token.is_empty() {
        settings.set_status(app.borrow().catalog.tr(19).into());
        return false;
    }
    let changed = cfg.url != app.borrow().cfg.url || cfg.token != app.borrow().cfg.token || cfg.device_name != app.borrow().cfg.device_name;
    if changed {
        cfg.webhook_id.clear();
    }
    if cfg.device_id.is_empty() {
        cfg.device_id = uuid::Uuid::new_v4().simple().to_string();
    }
    if let Err(err) = config::apply_autostart(cfg.autostart, &config::autostart_path()) {
        settings.set_status(app.borrow().catalog.trf(20, &[&err.to_string()]).into());
        return false;
    }
    app.borrow_mut().cfg = cfg;
    if config::save_config(&app.borrow().cfg, &config::config_path()).is_err() {
        return false;
    }
    settings.set_status(app.borrow().catalog.tr(21).into());
    start_client(app, tx);
    close
}
