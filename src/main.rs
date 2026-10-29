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
    popup_focused: bool,
    popup_seen_focus: bool,
    close_popup: bool,
    pinned: bool,
    editing: Option<usize>,
    popup_at: Instant,
    placer_ok: bool,
    place_warned: bool,
    status: String,
}

struct Shell {
    settings: Option<Settings>,
    popup: Option<Popup>,
    slide_timer: Option<Rc<slint::Timer>>,
    raise_timer: Option<slint::Timer>,
    title_timer: Option<slint::Timer>,
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
            popup_focused: false,
            popup_seen_focus: false,
            close_popup: false,
            pinned: false,
            editing: None,
            popup_at: Instant::now(),
            placer_ok,
            place_warned: false,
            status: String::new(),
        })),
        shell: Rc::new(RefCell::new(Shell {
            settings: None,
            popup: None,
            slide_timer: None,
            raise_timer: None,
            title_timer: None,
        })),
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
        present_settings(&ctx, &settings);
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
                    present_settings(&ctx_timer, &settings);
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
                ClientEv::Status(id) => {
                    let text = ctx_timer.app.borrow().catalog.tr(id);
                    note_status(&ctx_timer, text);
                }
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
                ClientEv::Notify { title, message, image } => {
                    let icon = icons::asset_path("icon.png");
                    let icon = icon.display().to_string();
                    let base = ctx_timer.app.borrow().cfg.url.clone();
                    let token = ctx_timer.app.borrow().cfg.token.clone();
                    ctx_timer.rt.spawn(async move {
                        let photo = match image {
                            Some(image) => notify::fetch_image(&base, &token, &image).await,
                            None => None,
                        };
                        notify::announce(&title, &message, &icon, photo.as_deref()).await;
                    });
                }
                ClientEv::Linked(up) => {
                    let id = if up { 22 } else { 24 };
                    let text = ctx_timer.app.borrow().catalog.tr(id);
                    note_status(&ctx_timer, text);
                }
            }
        }
        if ctx_timer.app.borrow().close_popup {
            ctx_timer.app.borrow_mut().close_popup = false;
            hide_popup(&ctx_timer);
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
    let focus = ctx_from(ctx);
    let _ = popup.window().on_winit_window_event(move |_window, event| {
        match event {
            winit::event::WindowEvent::ScaleFactorChanged { .. } => {
                if let Some(popup) = weak.upgrade() {
                    let _ = sharpen_popup(&popup);
                }
            }
            winit::event::WindowEvent::Focused(true) => {
                let mut app = focus.app.borrow_mut();
                app.popup_focused = true;
                app.popup_seen_focus = true;
            }
            winit::event::WindowEvent::Focused(false) => {
                let mut app = focus.app.borrow_mut();
                let close = app.popup_seen_focus
                    && app.popup_open
                    && !app.pinned
                    && app.popup_at.elapsed() >= Duration::from_millis(350);
                app.popup_focused = false;
                if close {
                    app.close_popup = true;
                }
            }
            winit::event::WindowEvent::KeyboardInput { event, .. }
                if event.state == winit::event::ElementState::Pressed
                    && event.logical_key == winit::keyboard::Key::Named(winit::keyboard::NamedKey::Escape) =>
            {
                let mut app = focus.app.borrow_mut();
                if app.popup_open && !app.pinned {
                    app.close_popup = true;
                }
            }
            _ => {}
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

fn hide_popup(ctx: &Ctx) {
    if let Some(timer) = ctx.shell.borrow().slide_timer.clone() {
        timer.stop();
    }
    if let Some(popup) = popup_of(&ctx.shell) {
        popup.set_slide(0.0);
        popup.hide().ok();
    }
    let mut app = ctx.app.borrow_mut();
    app.popup_open = false;
    app.popup_focused = false;
    app.popup_seen_focus = false;
    app.close_popup = false;
}

fn start_slide(ctx: &Ctx, popup: &Popup) {
    if let Some(timer) = ctx.shell.borrow().slide_timer.clone() {
        timer.stop();
    }
    let height = popup.get_layout_height();
    popup.set_slide(if height > 1.0 { height } else { 0.0 });
    let weak = popup.as_weak();
    let start = Rc::new(Cell::new(None::<Instant>));
    let timer = Rc::new(slint::Timer::default());
    let tick = timer.clone();
    timer.start(slint::TimerMode::Repeated, Duration::from_millis(16), move || {
        let Some(popup) = weak.upgrade() else {
            tick.stop();
            return;
        };
        let height = popup.get_layout_height();
        if height < 2.0 {
            return;
        }
        if start.get().is_none() {
            start.set(Some(Instant::now()));
        }
        let elapsed = start.get().map(|at| at.elapsed().as_secs_f32()).unwrap_or(0.0);
        let t = (elapsed / 0.18).min(1.0);
        let ease = 1.0 - (1.0 - t).powi(3);
        popup.set_slide(height * (1.0 - ease));
        if t >= 1.0 {
            popup.set_slide(0.0);
            tick.stop();
        }
    });
    ctx.shell.borrow_mut().slide_timer = Some(timer);
}

fn present_settings(ctx: &Ctx, settings: &Settings) {
    dismiss_popup_if_loose(ctx);
    settings.show().ok();
    raise_settings(settings);
    let weak = settings.as_weak();
    let kick = slint::Timer::default();
    kick.start(slint::TimerMode::SingleShot, Duration::from_millis(50), move || {
        if let Some(settings) = weak.upgrade() {
            raise_settings(&settings);
        }
    });
    let weak = settings.as_weak();
    let back = slint::Timer::default();
    back.start(slint::TimerMode::SingleShot, Duration::from_millis(220), move || {
        if let Some(settings) = weak.upgrade() {
            settings.set_caption("yoin".into());
        }
    });
    let mut shell = ctx.shell.borrow_mut();
    shell.raise_timer = Some(kick);
    shell.title_timer = Some(back);
}

fn raise_settings(settings: &Settings) {
    settings.set_caption("yoin\u{200b}".into());
    let _ = settings.window().with_winit_window(|window| {
        window.set_minimized(false);
        window.focus_window();
    });
}

fn dismiss_popup_if_loose(ctx: &Ctx) {
    let loose = {
        let app = ctx.app.borrow();
        app.popup_open && !app.pinned
    };
    if loose {
        hide_popup(ctx);
    }
}

fn open_home(ctx: &Ctx) {
    let url = ctx.app.borrow().cfg.url.trim().to_string();
    if url.is_empty() {
        let text = ctx.app.borrow().catalog.tr(19);
        note_status(ctx, text);
        return;
    }
    dismiss_popup_if_loose(ctx);
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
}

fn toggle_popup(ctx: &Ctx) -> bool {
    let popup = ensure_popup(ctx);
    if ctx.app.borrow().popup_open {
        hide_popup(ctx);
        return false;
    }
    refresh_popup(&ctx.app, &popup);
    {
        let mut app = ctx.app.borrow_mut();
        app.popup_open = true;
        app.popup_focused = false;
        app.popup_seen_focus = false;
        app.close_popup = false;
        app.popup_at = Instant::now();
    }
    popup.set_pinned(ctx.app.borrow().pinned);
    let height = popup.get_layout_height();
    popup.set_slide(if height > 1.0 { height } else { 0.0 });
    popup.show().ok();
    let _ = popup.window().with_winit_window(|window| {
        window.set_title("yoin-buttons");
        window.focus_window();
    });
    let _ = sharpen_popup(&popup);
    start_slide(ctx, &popup);
    if !ctx.app.borrow().placer_ok && !ctx.app.borrow().place_warned {
        ctx.app.borrow_mut().place_warned = true;
        let text = ctx.app.borrow().catalog.tr(23);
        note_status(ctx, text);
    }
    true
}

fn refresh_popup(app: &Rc<RefCell<App>>, popup: &Popup) {
    let app = app.borrow();
    let unset = app.catalog.tr(12);
    let tiles = app.cfg.layout == "tiles";
    let faces: Vec<Face> = app.cfg.buttons.iter().enumerate().map(|(index, id)| {
        let known = app.entities.get(id);
        let custom = app.cfg.labels.get(index).map(String::as_str).unwrap_or("");
        Face {
            text: ha::tray_label(id, known, custom, &unset, tiles).into(),
            on: ha::button_on(known),
            kind: ha::entity_kind(id, known).into(),
        }
    }).collect();
    popup.set_tiles(tiles);
    popup.set_button_size(app.cfg.button_size as f32);
    popup.set_pinned(app.pinned);
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
    settings.set_t_layout(cat.tr(34).into());
    settings.set_t_size(cat.tr(37).into());
    settings.set_t_size_hint(cat.tr(39).into());
    settings.set_t_rename(cat.tr(38).into());
    settings.set_version(env!("CARGO_PKG_VERSION").into());
    settings.set_address(app_ref.cfg.url.clone().into());
    settings.set_token(app_ref.cfg.token.clone().into());
    settings.set_device(app_ref.cfg.device_name.clone().into());
    settings.set_autostart(app_ref.cfg.autostart);
    let names = i18n::languages(&i18n::language_dir());
    let index = names.iter().position(|n| n == &app_ref.cfg.language).unwrap_or(0) as i32;
    let language_names: Vec<slint::SharedString> = names.into_iter().map(Into::into).collect();
    let theme_names = vec![cat.tr(15).into(), cat.tr(16).into(), cat.tr(17).into()];
    let theme_index = match app_ref.cfg.theme.as_str() {
        "light" => 1,
        "dark" => 2,
        _ => 0,
    };
    let layout_names = vec![cat.tr(35).into(), cat.tr(36).into()];
    let layout_index = if app_ref.cfg.layout == "tiles" { 1 } else { 0 };
    let button_size = app_ref.cfg.button_size as f32;
    drop(app_ref);
    settings.set_language_names(ModelRc::new(VecModel::from(language_names)));
    settings.set_language_index(index);
    settings.set_theme_names(ModelRc::new(VecModel::from(theme_names)));
    settings.set_theme_index(theme_index);
    settings.set_layout_names(ModelRc::new(VecModel::from(layout_names)));
    settings.set_layout_index(layout_index);
    settings.set_button_size_value(button_size);
}

fn fill_rows(app: &Rc<RefCell<App>>, settings: &Settings) {
    let app_ref = app.borrow();
    let unset = app_ref.catalog.tr(12);
    let mut labels = vec![unset.into()];
    labels.extend(app_ref.names.iter().map(|e| e.name.clone().into()));
    settings.set_entity_names(ModelRc::new(VecModel::from(labels)));
    let rows: Vec<Choice> = app_ref.cfg.buttons.iter().enumerate().map(|(slot, id)| {
        let index = if id.is_empty() {
            0
        } else {
            app_ref.names.iter().position(|e| e.entity_id == *id).map(|i| i as i32 + 1).unwrap_or(0)
        };
        let label = app_ref.cfg.labels.get(slot).cloned().unwrap_or_default();
        Choice { index, label: label.into(), editing: app_ref.editing == Some(slot) }
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
        app_a.borrow_mut().cfg.labels.push(String::new());
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
            if index < cfg_app.cfg.labels.len() {
                cfg_app.cfg.labels.remove(index);
            }
        }
        if cfg_app.editing == Some(index) {
            cfg_app.editing = None;
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
        if row < borrowed.cfg.labels.len() {
            borrowed.cfg.labels[row].clear();
        }
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
    let app_e = ctx.app.clone();
    let settings_e = settings.as_weak();
    settings.on_edit(move |index| {
        let index = index as usize;
        let mut app = app_e.borrow_mut();
        if index >= app.cfg.buttons.len() {
            return;
        }
        app.editing = if app.editing == Some(index) { None } else { Some(index) };
        drop(app);
        if let Some(settings) = settings_e.upgrade() {
            fill_rows(&app_e, &settings);
        }
    });
    let app_n = ctx.app.clone();
    let shell_n = ctx.shell.clone();
    let settings_n = settings.as_weak();
    settings.on_rename(move |index, text| {
        let index = index as usize;
        let text = text.trim().to_string();
        {
            let mut app = app_n.borrow_mut();
            if index >= app.cfg.buttons.len() {
                return;
            }
            config::align_labels(&mut app.cfg);
            if app.cfg.labels.get(index).map(String::as_str) == Some(text.as_str()) {
                app.editing = None;
                drop(app);
                if let Some(settings) = settings_n.upgrade() {
                    fill_rows(&app_n, &settings);
                }
                return;
            }
            app.cfg.labels[index] = text;
            app.editing = None;
        }
        let _ = config::save_config(&app_n.borrow().cfg, &config::config_path());
        if let Some(settings) = settings_n.upgrade() {
            fill_rows(&app_n, &settings);
        }
        if let Some(popup) = popup_of(&shell_n) {
            refresh_popup(&app_n, &popup);
        }
    });
    let ctx_y = ctx_from(ctx);
    settings.on_layout(move |index| {
        let layout = if index == 1 { "tiles" } else { "list" };
        if ctx_y.app.borrow().cfg.layout == layout {
            return;
        }
        ctx_y.app.borrow_mut().cfg.layout = layout.into();
        let _ = config::save_config(&ctx_y.app.borrow().cfg, &config::config_path());
        if let Some(popup) = popup_of(&ctx_y.shell) {
            refresh_popup(&ctx_y.app, &popup);
        }
    });
    let ctx_z = ctx_from(ctx);
    settings.on_size_changed(move |value| {
        let size = value.round().clamp(32.0, 80.0) as u32;
        if ctx_z.app.borrow().cfg.button_size == size {
            return;
        }
        ctx_z.app.borrow_mut().cfg.button_size = size;
        let _ = config::save_config(&ctx_z.app.borrow().cfg, &config::config_path());
        if let Some(popup) = popup_of(&ctx_z.shell) {
            refresh_popup(&ctx_z.app, &popup);
        }
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
            present_settings(&ctx_p, &settings);
            return;
        }
        let state = app_p.borrow().entities.get(&entity).map(|e| e.state.clone()).unwrap_or_default();
        let _ = tx.send(ClientCmd::Call { entity, state });
    });
    let ctx_c = ctx_from(ctx);
    popup.on_configure(move || {
        let settings = ensure_settings(&ctx_c);
        present_settings(&ctx_c, &settings);
    });
    let ctx_h = ctx_from(ctx);
    popup.on_open_home(move || open_home(&ctx_h));
    let ctx_pin = ctx_from(ctx);
    popup.on_pin_toggled(move || {
        let next = !ctx_pin.app.borrow().pinned;
        ctx_pin.app.borrow_mut().pinned = next;
        if let Some(popup) = popup_of(&ctx_pin.shell) {
            popup.set_pinned(next);
        }
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
            if index < borrowed.cfg.labels.len() {
                borrowed.cfg.labels.swap(index, index - 1);
            }
        } else if index + 1 >= len {
            return;
        } else {
            borrowed.cfg.buttons.swap(index, index + 1);
            if index + 1 < borrowed.cfg.labels.len() {
                borrowed.cfg.labels.swap(index, index + 1);
            }
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
