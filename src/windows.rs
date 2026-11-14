// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-or-later

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use futures_util::future::{AbortHandle, Abortable};
use futures_util::lock::Mutex;
use slint::winit_030::WinitWindowAccessor;
use slint::ComponentHandle;

thread_local! {
    static CREATION: Rc<Mutex<()>> = Rc::new(Mutex::new(()));
}

#[derive(Clone, Default)]
pub struct Visibility(Rc<RefCell<Option<(AbortHandle, Rc<Cell<bool>>)>>>);

struct ClassGuard;

impl Drop for ClassGuard {
    fn drop(&mut self) {
        let _ = slint::set_xdg_app_id("yoin");
    }
}

impl Visibility {
    pub fn cancel(&self) -> bool {
        if let Some((task, creating)) = self.0.borrow_mut().take() {
            task.abort();
            return creating.get();
        }
        false
    }

    pub fn show<T: ComponentHandle + 'static>(&self, component: &T, class: &'static str) {
        if self.0.borrow().is_some() {
            return;
        }
        let weak = component.as_weak();
        let creation = CREATION.with(Rc::clone);
        let (abort, registration) = AbortHandle::new_pair();
        let creating = Rc::new(Cell::new(false));
        *self.0.borrow_mut() = Some((abort.clone(), creating.clone()));
        let pending = self.0.clone();
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
            if component.show().is_ok() {
                if let Err(error) = component.window().winit_window().await {
                    eprintln!("yoin: window: {error}");
                }
            }
            // Even a cancelled creation keeps its class until the native window exists.
            if abort.is_aborted() {
                component.hide().ok();
            } else {
                pending.borrow_mut().take();
            }
        });
    }

    pub fn hide<T: ComponentHandle>(&self, component: &T) {
        if !self.cancel() {
            component.hide().ok();
        }
    }
}
