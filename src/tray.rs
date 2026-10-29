// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-or-later

use std::sync::{Arc, Mutex};

use tokio::sync::mpsc;

#[derive(Clone)]
pub struct TrayView {
    pub settings: String,
    pub quit: String,
    pub icon: Vec<ksni::Icon>,
}

pub enum TrayCmd {
    Toggle,
    Settings,
}

pub struct YoinTray {
    pub view: Arc<Mutex<TrayView>>,
    pub tx: mpsc::UnboundedSender<TrayCmd>,
}

impl ksni::Tray for YoinTray {
    fn id(&self) -> String {
        "yoin".into()
    }
    fn title(&self) -> String {
        "yoin".into()
    }
    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        self.view.lock().map(|view| view.icon.clone()).unwrap_or_default()
    }
    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        let view = self.view.lock().ok();
        let settings = view.as_ref().map(|v| v.settings.clone()).unwrap_or_else(|| "Settings".into());
        let quit = view.as_ref().map(|v| v.quit.clone()).unwrap_or_else(|| "Quit".into());
        vec![
            ksni::menu::StandardItem {
                label: settings,
                activate: Box::new(|this: &mut Self| {
                    let _ = this.tx.send(TrayCmd::Settings);
                }),
                ..Default::default()
            }
            .into(),
            ksni::menu::StandardItem {
                label: quit,
                activate: Box::new(|_: &mut Self| std::process::exit(0)),
                ..Default::default()
            }
            .into(),
        ]
    }
    fn activate(&mut self, _x: i32, _y: i32) {
        let _ = self.tx.send(TrayCmd::Toggle);
    }
}
