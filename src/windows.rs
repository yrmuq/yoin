// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-or-later

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use futures_util::future::{AbortHandle, Abortable};
use futures_util::lock::Mutex;
use slint::winit_030::{winit, WinitWindowAccessor};
use slint::ComponentHandle;

pub const POPUP_CLASS: &str = "yoin-popup-buttons";
pub const NOTICES_CLASS: &str = "yoin-popup-notices";

thread_local! {
    static INITIAL_CLASS: Cell<&'static str> = const { Cell::new("") };
    static CREATION: Rc<Mutex<()>> = Rc::new(Mutex::new(()));
}

struct InitialClassGuard(&'static str);

impl InitialClassGuard {
    fn new(class: &'static str) -> Self {
        Self(INITIAL_CLASS.with(|current| current.replace(class)))
    }
}

impl Drop for InitialClassGuard {
    fn drop(&mut self) {
        INITIAL_CLASS.with(|current| current.set(self.0));
    }
}

fn panel_title(class: &str) -> Option<&'static str> {
    match class {
        POPUP_CLASS => Some("yoin-buttons"),
        NOTICES_CLASS => Some("yoin-notices"),
        _ => None,
    }
}

pub fn window_attributes(attributes: winit::window::WindowAttributes) -> winit::window::WindowAttributes {
    let attributes = attributes.with_transparent(true);
    INITIAL_CLASS.with(|class| match panel_title(class.get()) {
        Some(title) => attributes.with_title(title).with_decorations(false).with_resizable(false),
        None => attributes,
    })
}

pub fn new_panel<T: ComponentHandle>(class: &'static str, create: impl FnOnce() -> Result<T, slint::PlatformError>) -> Result<T, slint::PlatformError> {
    let _initial = InitialClassGuard::new(class);
    let component = create()?;
    // Force the adapter's attribute hook while the panel role is in scope.
    let _ = component.window();
    Ok(component)
}

pub async fn create<T: ComponentHandle>(class: &'static str, create: impl FnOnce() -> Option<T>) -> Option<T> {
    let creation = CREATION.with(Rc::clone);
    let _creation = creation.lock().await;
    // Adapters are registered for native creation before show() is polled.
    let _ = slint::set_xdg_app_id(class);
    let _class = ClassGuard;
    let component = create()?;
    if let Err(error) = component.window().winit_window().await {
        eprintln!("yoin: window: {error}");
    }
    Some(component)
}

#[derive(Clone, Default)]
pub struct Visibility {
    pending: Rc<RefCell<Option<(AbortHandle, Rc<Cell<bool>>)>>>,
    frame: Rc<RefCell<Option<tokio::sync::oneshot::Sender<()>>>>,
}

struct ClassGuard;

impl Drop for ClassGuard {
    fn drop(&mut self) {
        let _ = slint::set_xdg_app_id("yoin");
    }
}

impl Visibility {
    pub fn cancel(&self) -> bool {
        self.frame.borrow_mut().take();
        if let Some((task, creating)) = self.pending.borrow_mut().take() {
            task.abort();
            return creating.get();
        }
        false
    }

    pub fn redraw_requested(&self) {
        let Some(frame) = self.frame.borrow_mut().take() else { return };
        // The winit hook runs before draw; start only after the transparent frame is rendered.
        slint::Timer::single_shot(std::time::Duration::ZERO, move || { let _ = frame.send(()); });
    }

    pub fn show<T: ComponentHandle + 'static>(&self, component: &T, class: &'static str) {
        self.show_ready(component, class, |_| {});
    }

    pub fn pending(&self) -> bool {
        self.pending.borrow().is_some()
    }

    pub fn show_ready<T: ComponentHandle + 'static>(&self, component: &T, class: &'static str, ready: impl FnOnce(T) + 'static) {
        if self.pending() { return; }
        let weak = component.as_weak();
        let creation = CREATION.with(Rc::clone);
        let (abort, registration) = AbortHandle::new_pair();
        let creating = Rc::new(Cell::new(false));
        *self.pending.borrow_mut() = Some((abort.clone(), creating.clone()));
        let pending = self.pending.clone();
        let frame = self.frame.clone();
        let _ = slint::spawn_local(async move {
            let Ok(_creation) = Abortable::new(creation.lock(), registration).await else { return };
            let Some(component) = weak.upgrade() else {
                pending.borrow_mut().take();
                return;
            };
            // Slint creates native windows later and shares the app ID between them.
            let _ = slint::set_xdg_app_id(class);
            let _class = ClassGuard;
            creating.set(true);
            let (rendered, first_frame) = tokio::sync::oneshot::channel();
            *frame.borrow_mut() = Some(rendered);
            let shown = match component.show() {
                Ok(()) => match component.window().winit_window().await {
                    Ok(window) => {
                        // Slint restores default attributes when recreating a Wayland window.
                        if let Some(title) = panel_title(class) {
                            window.set_title(title);
                            window.set_decorations(false);
                            window.set_resizable(false);
                        }
                        true
                    },
                    Err(error) => {
                        eprintln!("yoin: window: {error}");
                        false
                    }
                },
                Err(error) => {
                    eprintln!("yoin: window: {error}");
                    false
                }
            };
            // Even a cancelled creation keeps its class until the native window exists.
            let rendered = if shown {
                component.window().request_redraw();
                first_frame.await.is_ok()
            } else {
                frame.borrow_mut().take();
                false
            };
            if abort.is_aborted() {
                component.hide().ok();
            } else {
                pending.borrow_mut().take();
                if rendered { ready(component); }
            }
        });
    }

    pub fn hide<T: ComponentHandle>(&self, component: &T) {
        if !self.cancel() {
            component.hide().ok();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cold_panel_attributes_are_frameless_without_affecting_settings() {
        for (class, title) in [(POPUP_CLASS, "yoin-buttons"), (NOTICES_CLASS, "yoin-notices")] {
            let _initial = InitialClassGuard::new(class);
            let attributes = window_attributes(winit::window::WindowAttributes::default());
            assert!(!attributes.decorations);
            assert!(!attributes.resizable);
            assert!(attributes.transparent);
            assert_eq!(attributes.title, title);
        }
        let settings = window_attributes(winit::window::WindowAttributes::default().with_title("yoin"));
        assert!(settings.decorations);
        assert!(settings.resizable);
        assert_eq!(settings.title, "yoin");
    }

    #[tokio::test]
    async fn cached_window_does_not_wait_for_native_recreation() {
        let existing = create::<crate::Settings>("yoin", || None).await;
        assert!(existing.is_none());
    }

    #[tokio::test]
    async fn cancel_unblocks_a_window_waiting_for_its_first_frame() {
        let visibility = Visibility::default();
        let (abort, _) = AbortHandle::new_pair();
        let (frame, waiting) = tokio::sync::oneshot::channel();
        *visibility.pending.borrow_mut() = Some((abort.clone(), Rc::new(Cell::new(true))));
        *visibility.frame.borrow_mut() = Some(frame);
        assert!(visibility.cancel());
        assert!(waiting.await.is_err());
        assert!(abort.is_aborted());
        assert!(!visibility.pending());
    }
}
