// SPDX-License-Identifier: GPL-3.0-or-later
//! StatusNotifier tray, implemented independently using ksni's public API.
use super::Event;
use std::sync::mpsc::Sender;
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
#[derive(Clone, Copy)]
pub enum Action {
    Show,
    Connect,
    Disconnect,
    Servers,
    Settings,
    Recover,
    Quit,
}
#[derive(Clone, Default, PartialEq)]
pub struct Snapshot {
    pub status: String,
    pub active: bool,
    pub signed_in: bool,
}

#[cfg(target_os = "linux")]
mod platform {
    use super::*;
    use ksni::{blocking::TrayMethods, menu::StandardItem, MenuItem};
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };
    pub struct Item {
        pub state: Snapshot,
        sender: Sender<Event>,
        visible: Arc<AtomicBool>,
    }
    impl Item {
        fn item(&self, label: &str, enabled: bool, action: Action) -> MenuItem<Self> {
            StandardItem {
                label: label.into(),
                enabled,
                activate: Box::new(move |s: &mut Self| {
                    let _ = s.sender.send(Event::Tray(action));
                }),
                ..Default::default()
            }
            .into()
        }
    }
    impl ksni::Tray for Item {
        fn id(&self) -> String {
            "wave-vpn".into()
        }
        fn title(&self) -> String {
            format!("Wave VPN — {}", self.state.status)
        }
        fn icon_pixmap(&self) -> Vec<ksni::Icon> {
            let color = match self.state.status.as_str() {
                "Connected" => [71, 120, 255],
                "Stalled" => [220, 69, 77],
                _ if self.state.active => [210, 143, 27],
                _ => [104, 112, 128],
            };
            [16, 22, 24, 32, 48]
                .into_iter()
                .map(|size| ksni::Icon {
                    width: size as i32,
                    height: size as i32,
                    data: crate::tray_icon::pixels(size, color),
                })
                .collect()
        }
        fn activate(&mut self, _x: i32, _y: i32) {
            let _ = self.sender.send(Event::Tray(Action::Show));
        }
        fn menu(&self) -> Vec<MenuItem<Self>> {
            vec![
                self.item(
                    &format!("Wave VPN — {}", self.state.status),
                    false,
                    Action::Show,
                ),
                self.item("Show Wave VPN", true, Action::Show),
                self.item(
                    if self.state.active {
                        "Disconnect"
                    } else {
                        "Connect"
                    },
                    self.state.signed_in,
                    if self.state.active {
                        Action::Disconnect
                    } else {
                        Action::Connect
                    },
                ),
                MenuItem::Separator,
                self.item("Servers", true, Action::Servers),
                self.item("Settings", true, Action::Settings),
                self.item("Recover networking", !self.state.active, Action::Recover),
                MenuItem::Separator,
                self.item("Quit", true, Action::Quit),
            ]
        }
        fn watcher_online(&self) {
            self.visible.store(true, Ordering::Relaxed);
        }
        fn watcher_offline(&self, _reason: ksni::OfflineReason) -> bool {
            self.visible.store(false, Ordering::Relaxed);
            true
        }
    }
    pub struct Handle {
        inner: ksni::blocking::Handle<Item>,
        visible: Arc<AtomicBool>,
        last: Snapshot,
    }
    impl Handle {
        pub fn visible(&self) -> bool {
            !self.inner.is_closed() && self.visible.load(Ordering::Relaxed)
        }
        pub fn update(&mut self, state: Snapshot) {
            if state != self.last {
                self.last = state.clone();
                self.inner.update(|item| item.state = state);
            }
        }
    }
    impl Drop for Handle {
        fn drop(&mut self) {
            self.inner.shutdown();
        }
    }
    pub fn spawn(sender: Sender<Event>) -> Option<Handle> {
        let visible = Arc::new(AtomicBool::new(true));
        match (Item {
            state: Snapshot::default(),
            sender,
            visible: visible.clone(),
        })
        .spawn()
        {
            Ok(inner) => Some(Handle {
                inner,
                visible,
                last: Snapshot::default(),
            }),
            Err(e) => {
                eprintln!("Tray unavailable: {e}");
                None
            }
        }
    }
}
#[cfg(not(target_os = "linux"))]
mod platform {
    use super::*;
    pub struct Handle;
    impl Handle {
        pub fn visible(&self) -> bool {
            false
        }
        pub fn update(&mut self, _state: Snapshot) {}
    }
    pub fn spawn(_sender: Sender<Event>) -> Option<Handle> {
        None
    }
}
pub use platform::{spawn, Handle};
