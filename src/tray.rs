use eframe::egui;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tray_icon::menu::{Menu, MenuEvent, MenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use windows::core::w;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, IsIconic, IsWindowVisible, PostMessageW, SetForegroundWindow, ShowWindow, SW_HIDE,
    SW_RESTORE, SW_SHOW, WM_CLOSE,
};

pub struct TrayManager {
    _tray: TrayIcon,
}

fn find_main_hwnd() -> Option<HWND> {
    let hwnd = unsafe { FindWindowW(None, w!("Acer Display Widget (Rust)")) }.ok()?;
    if hwnd.0.is_null() {
        None
    } else {
        Some(hwnd)
    }
}

fn toggle_main_window(ctx: &egui::Context) {
    if let Some(hwnd) = find_main_hwnd() {
        unsafe {
            let visible = IsWindowVisible(hwnd).as_bool();
            let minimized = IsIconic(hwnd).as_bool();
            if visible && !minimized {
                let _ = ShowWindow(hwnd, SW_HIDE);
            } else {
                let _ = ShowWindow(hwnd, SW_SHOW);
                if minimized {
                    let _ = ShowWindow(hwnd, SW_RESTORE);
                }
                let _ = SetForegroundWindow(hwnd);
                ctx.request_repaint();
            }
        }
    }
}

fn quit_main_window(ctx: &egui::Context, quitting: &AtomicBool) {
    quitting.store(true, Ordering::SeqCst);
    if let Some(hwnd) = find_main_hwnd() {
        unsafe {
            let _ = PostMessageW(hwnd, WM_CLOSE, WPARAM(0), LPARAM(0));
        }
    }
    ctx.request_repaint();
    // 保底機制：若視窗隱藏且事件迴圈未在 350ms 內結束，強制退出
    std::thread::spawn(|| {
        std::thread::sleep(Duration::from_millis(350));
        std::process::exit(0);
    });
}

impl TrayManager {
    pub fn new(ctx: egui::Context, quitting: Arc<AtomicBool>) -> Result<Self, String> {
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

        let ctx_icon = ctx.clone();
        TrayIconEvent::set_event_handler(Some(move |ev| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = ev
            {
                toggle_main_window(&ctx_icon);
            }
        }));

        MenuEvent::set_event_handler(Some(move |ev: MenuEvent| {
            if ev.id == toggle_id {
                toggle_main_window(&ctx);
            } else if ev.id == quit_id {
                quit_main_window(&ctx, &quitting);
            }
        }));

        Ok(Self { _tray: tray })
    }
}

pub fn app_icon_rgba() -> (Vec<u8>, u32, u32) {
    (include_bytes!("../assets/icon_64.rgba").to_vec(), 64, 64)
}

fn build_monitor_icon() -> Icon {
    let (rgba, w, h) = app_icon_rgba();
    Icon::from_rgba(rgba, w, h).expect("valid tray icon")
}
