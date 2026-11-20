use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

pub enum TrayAction {
    ToggleWindow,
    Quit,
}

pub struct TrayManager {
    _tray: TrayIcon,
    toggle_id: MenuId,
    quit_id: MenuId,
}

impl TrayManager {
    pub fn new() -> Result<Self, String> {
        let menu = Menu::new();
        let toggle_item = MenuItem::new("顯示 / 隱藏視窗", true, None);
        let quit_item = MenuItem::new("結束程式", true, None);
        let toggle_id = toggle_item.id().clone();
        let quit_id = quit_item.id().clone();

        menu.append(&toggle_item).map_err(|e| e.to_string())?;
        menu.append(&quit_item).map_err(|e| e.to_string())?;

        let icon = build_monitor_icon();
        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("Acer Display Widget (Rust)")
            .with_icon(icon)
            .build()
            .map_err(|e| e.to_string())?;

        Ok(Self {
            _tray: tray,
            toggle_id,
            quit_id,
        })
    }

    pub fn poll(&self) -> Option<TrayAction> {
        while let Ok(ev) = TrayIconEvent::receiver().try_recv() {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = ev
            {
                return Some(TrayAction::ToggleWindow);
            }
        }
        while let Ok(ev) = MenuEvent::receiver().try_recv() {
            if ev.id == self.toggle_id {
                return Some(TrayAction::ToggleWindow);
            }
            if ev.id == self.quit_id {
                return Some(TrayAction::Quit);
            }
        }
        None
    }
}

fn build_monitor_icon() -> Icon {
    let size = 32usize;
    let mut rgba = vec![0u8; size * size * 4];
    let set_px = |buf: &mut [u8], x: usize, y: usize, r: u8, g: u8, b: u8, a: u8| {
        let i = (y * size + x) * 4;
        buf[i] = r;
        buf[i + 1] = g;
        buf[i + 2] = b;
        buf[i + 3] = a;
    };

    // Monitor bezel & blue screen
    for y in 4..23 {
        for x in 2..30 {
            let border = y <= 5 || y >= 21 || x <= 3 || x >= 28;
            if border {
                set_px(&mut rgba, x, y, 220, 226, 236, 255);
            } else {
                set_px(&mut rgba, x, y, 28, 102, 224, 255);
            }
        }
    }
    // Stand neck
    for y in 23..27 {
        for x in 13..19 {
            set_px(&mut rgba, x, y, 200, 206, 216, 255);
        }
    }
    // Stand base
    for y in 27..29 {
        for x in 9..23 {
            set_px(&mut rgba, x, y, 220, 226, 236, 255);
        }
    }

    Icon::from_rgba(rgba, size as u32, size as u32).expect("valid tray icon")
}
