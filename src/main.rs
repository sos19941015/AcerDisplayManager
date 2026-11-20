#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod icons;
mod mccs;
mod profiles;
mod tray;
mod win_oled;

use eframe::egui;
use mccs::{
    enumerate_displays, CustomSettings, DisplayInfo, DisplayMode, MccsSdk, OledHwState,
    SystemHwState, VCP_HUE, VCP_RGB_BLACK, VCP_SAT, VCP_SHARPNESS,
};
use profiles::{load_profiles, save_profiles, CustomProfile};
use tray::{TrayAction, TrayManager};
use win_oled::{
    get_auto_hdr, get_display_resolution, get_monitor_edid_info, get_startup_enabled,
    read_win_oled_state, set_auto_hdr, set_dark_mode, set_screensaver, set_startup_enabled,
    set_taskbar_autohide, set_transparency, set_windows_hdr, WinOledState,
};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Page {
    Main,
    Custom,
    Profiles,
    OledCare,
    SystemSettings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SliderTarget {
    Brightness,
    Contrast,
    RGain,
    GGain,
    BGain,
    RBlack,
    GBlack,
    BBlack,
    Sharpness,
    Sat6(usize),
    Hue6(usize),
    BlackBoost,
    Volume,
}

enum WorkerCmd {
    InitMonitor(DisplayInfo),
    SetDisplayMode {
        mode: DisplayMode,
        auto_hdr_device: Option<String>,
    },
    SetSlider(SliderTarget, u32),
    SetBlueLight(u32),
    SetGamma(u32),
    SetColorTemp(u32),
    SetInputSource(u32),
    ResetSat6,
    ResetHue6,
    SetRefreshRate(bool),
    SetOd(u32),
    SetAimType(u32),
    LoadCustom,
    LoadOled,
    TriggerPixelRefresh,
    SetPixelShift(bool),
    SetLogoDetection(bool),
    SetStaticImage(bool),
    SetAiProtection(bool),
    LoadSystem,
    SetPower(bool),
    SetPowerIndicator(bool),
    SetPowerKeyLock(bool),
    SetKeyLock(bool),
    SetUsbPort(u32),
    ApplyProfile {
        profile: CustomProfile,
        auto_hdr_device: Option<String>,
    },
    Refresh,
}

enum WorkerEvent {
    SdkLoaded(Result<String, String>),
    MonitorState {
        connected: bool,
        brightness: Option<(u32, u32, u32)>,
        mode: Option<DisplayMode>,
        status: String,
    },
    CustomLoaded(CustomSettings),
    OledLoaded(OledHwState),
    SystemLoaded {
        hw: SystemHwState,
        serial: Option<String>,
    },
    ActionResult(String),
}

fn spawn_worker(ctx: egui::Context) -> (Sender<WorkerCmd>, Receiver<WorkerEvent>) {
    let (cmd_tx, cmd_rx) = mpsc::channel::<WorkerCmd>();
    let (evt_tx, evt_rx) = mpsc::channel::<WorkerEvent>();

    thread::spawn(move || {
        let sdk = match MccsSdk::find_and_load() {
            Ok((sdk, path)) => {
                let _ = evt_tx.send(WorkerEvent::SdkLoaded(Ok(format!(
                    "已載入 {}",
                    path.display()
                ))));
                Some(sdk)
            }
            Err(err) => {
                let _ = evt_tx.send(WorkerEvent::SdkLoaded(Err(err)));
                None
            }
        };
        ctx.request_repaint();

        let query_state = |sdk: &MccsSdk| -> (Option<(u32, u32, u32)>, Option<DisplayMode>) {
            if !sdk.is_initialized() {
                return (None, None);
            }
            let b = sdk.get_brightness();
            let m = sdk.get_display_mode().and_then(DisplayMode::from_u32);
            (b, m)
        };

        let reinit_monitor = |sdk: &MccsSdk, current: &Option<DisplayInfo>| -> bool {
            let Some(info) = current else {
                return false;
            };
            for attempt in 0..6 {
                thread::sleep(Duration::from_millis(if attempt == 0 { 1000 } else { 500 }));
                if sdk.init(info.index, &info.device_name) {
                    return true;
                }
            }
            false
        };

        let mut current_display: Option<DisplayInfo> = None;
        let mut cached_custom = CustomSettings::default();

        while let Ok(cmd) = cmd_rx.recv() {
            let Some(sdk) = sdk.as_ref() else {
                continue;
            };
            match cmd {
                WorkerCmd::InitMonitor(info) => {
                    current_display = Some(info.clone());
                    let ok = sdk.init(info.index, &info.device_name);
                    if ok {
                        let (b, m) = query_state(sdk);
                        let _ = evt_tx.send(WorkerEvent::MonitorState {
                            connected: true,
                            brightness: b,
                            mode: m,
                            status: format!("已連線: {} ({})", info.device_name, info.friendly_name),
                        });
                    } else {
                        let _ = evt_tx.send(WorkerEvent::MonitorState {
                            connected: false,
                            brightness: None,
                            mode: None,
                            status: format!(
                                "init_MCCS 失敗 (不支援 DDC/CI 或非 Acer 螢幕): {}",
                                info.device_name
                            ),
                        });
                    }
                }
                WorkerCmd::Refresh => {
                    if !sdk.is_initialized() {
                        if let Some(info) = &current_display {
                            let _ = sdk.init(info.index, &info.device_name);
                        }
                    }
                    let (b, m) = query_state(sdk);
                    let connected = sdk.is_initialized() && (b.is_some() || m.is_some());
                    let _ = evt_tx.send(WorkerEvent::MonitorState {
                        connected,
                        brightness: b,
                        mode: m,
                        status: if connected {
                            "已重新讀取螢幕狀態".to_string()
                        } else {
                            "重新連線螢幕失敗，請稍後再按重新讀取".to_string()
                        },
                    });
                }
                WorkerCmd::LoadCustom => {
                    let settings = sdk.read_custom_settings();
                    cached_custom = settings.clone();
                    let _ = evt_tx.send(WorkerEvent::CustomLoaded(settings));
                    let _ = evt_tx.send(WorkerEvent::ActionResult("已讀取自訂模式參數".to_string()));
                }
                WorkerCmd::LoadOled => {
                    let hw = sdk.read_oled_hw_state();
                    let _ = evt_tx.send(WorkerEvent::OledLoaded(hw));
                }
                WorkerCmd::TriggerPixelRefresh => {
                    let ok = sdk.trigger_pixel_refresh();
                    let _ = evt_tx.send(WorkerEvent::ActionResult(if ok {
                        "已觸發螢幕像素重新整理".to_string()
                    } else {
                        "此螢幕不支援硬體像素重新整理".to_string()
                    }));
                }
                WorkerCmd::SetPixelShift(v) => {
                    let ok = sdk.set_pixel_shift(v);
                    let _ = evt_tx.send(WorkerEvent::ActionResult(if ok {
                        format!("像素移位已{}", if v { "開啟" } else { "關閉" })
                    } else {
                        "設定像素移位失敗".to_string()
                    }));
                }
                WorkerCmd::SetLogoDetection(v) => {
                    let ok = sdk.set_logo_detection(v);
                    let _ = evt_tx.send(WorkerEvent::ActionResult(if ok {
                        format!("標誌檢測已{}", if v { "開啟" } else { "關閉" })
                    } else {
                        "設定標誌檢測失敗".to_string()
                    }));
                }
                WorkerCmd::SetStaticImage(v) => {
                    let ok = sdk.set_static_image(v);
                    let _ = evt_tx.send(WorkerEvent::ActionResult(if ok {
                        format!("靜態圖像檢測已{}", if v { "開啟" } else { "關閉" })
                    } else {
                        "設定靜態圖像檢測失敗".to_string()
                    }));
                }
                WorkerCmd::SetAiProtection(v) => {
                    let ok = sdk.set_ai_protection(v);
                    let _ = evt_tx.send(WorkerEvent::ActionResult(if ok {
                        format!("人體存在感測已{}", if v { "開啟" } else { "關閉" })
                    } else {
                        "設定人體存在感測失敗".to_string()
                    }));
                }
                WorkerCmd::LoadSystem => {
                    let hw = sdk.read_system_hw_state();
                    let serial = sdk.read_serial_number();
                    let _ = evt_tx.send(WorkerEvent::SystemLoaded { hw, serial });
                }
                WorkerCmd::SetPower(on) => {
                    let ok = sdk.set_power_on(on);
                    let _ = evt_tx.send(WorkerEvent::ActionResult(if ok {
                        format!("螢幕電源已{}", if on { "開啟" } else { "關閉" })
                    } else {
                        "設定螢幕電源失敗".to_string()
                    }));
                }
                WorkerCmd::SetPowerIndicator(on) => {
                    let ok = sdk.set_power_indicator(on);
                    let _ = evt_tx.send(WorkerEvent::ActionResult(if ok {
                        format!("電源指示燈已{}", if on { "開啟" } else { "關閉" })
                    } else {
                        "設定電源指示燈失敗".to_string()
                    }));
                }
                WorkerCmd::SetPowerKeyLock(locked) => {
                    let ok = sdk.set_power_key_lock(locked);
                    let _ = evt_tx.send(WorkerEvent::ActionResult(if ok {
                        format!("電源按鍵鎖定已{}", if locked { "開啟" } else { "關閉" })
                    } else {
                        "設定電源按鍵鎖定失敗".to_string()
                    }));
                }
                WorkerCmd::SetKeyLock(locked) => {
                    let ok = sdk.set_key_lock(locked);
                    let _ = evt_tx.send(WorkerEvent::ActionResult(if ok {
                        format!("按鍵鎖定已{}", if locked { "開啟" } else { "關閉" })
                    } else {
                        "設定按鍵鎖定失敗".to_string()
                    }));
                }
                WorkerCmd::SetUsbPort(port) => {
                    let ok = sdk.set_usb_port(port);
                    let _ = evt_tx.send(WorkerEvent::ActionResult(if ok {
                        format!("USB Port 已切換 ({port})")
                    } else {
                        "設定 USB Port 失敗".to_string()
                    }));
                }
                WorkerCmd::SetSlider(target, val) => {
                    let ok = match target {
                        SliderTarget::Brightness => sdk.set_brightness(val),
                        SliderTarget::Contrast => sdk.set_contrast(val),
                        SliderTarget::RGain => sdk.set_rgain(val),
                        SliderTarget::GGain => sdk.set_ggain(val),
                        SliderTarget::BGain => sdk.set_bgain(val),
                        SliderTarget::RBlack => {
                            cached_custom.r_black.1 = val;
                            sdk.set_vcp(VCP_RGB_BLACK[0], val)
                        }
                        SliderTarget::GBlack => {
                            cached_custom.g_black.1 = val;
                            sdk.set_vcp(VCP_RGB_BLACK[1], val)
                        }
                        SliderTarget::BBlack => {
                            cached_custom.b_black.1 = val;
                            sdk.set_vcp(VCP_RGB_BLACK[2], val)
                        }
                        SliderTarget::Sharpness => {
                            cached_custom.sharpness.0 = val;
                            sdk.set_vcp(VCP_SHARPNESS, val)
                        }
                        SliderTarget::Sat6(idx) => {
                            cached_custom.sat_6[idx] = val;
                            sdk.set_vcp(VCP_SAT[idx], val)
                        }
                        SliderTarget::Hue6(idx) => {
                            cached_custom.hue_6[idx] = val;
                            sdk.set_vcp(VCP_HUE[idx], val)
                        }
                        SliderTarget::BlackBoost => sdk.set_blackboost(val),
                        SliderTarget::Volume => sdk.set_volume(val),
                    };
                    let _ = evt_tx.send(WorkerEvent::ActionResult(if ok {
                        format!("{target:?} 已設為 {val}")
                    } else {
                        format!("設定 {target:?} 失敗 ({val})")
                    }));
                }
                WorkerCmd::SetInputSource(v) => {
                    let ok = sdk.set_vcp(0x60, v);
                    let _ = evt_tx.send(WorkerEvent::ActionResult(if ok {
                        format!("訊號輸入源已切換 ({v})")
                    } else {
                        "切換訊號輸入源失敗".to_string()
                    }));
                }
                WorkerCmd::ResetSat6 => {
                    for &code in &VCP_SAT {
                        let _ = sdk.set_vcp(code, 50);
                    }
                    cached_custom.sat_6 = [50; 6];
                    let _ = evt_tx.send(WorkerEvent::ActionResult("6 軸飽和度已重置為 50".to_string()));
                }
                WorkerCmd::ResetHue6 => {
                    for &code in &VCP_HUE {
                        let _ = sdk.set_vcp(code, 50);
                    }
                    cached_custom.hue_6 = [50; 6];
                    let _ = evt_tx.send(WorkerEvent::ActionResult("6 軸色調已重置為 50".to_string()));
                }
                WorkerCmd::SetBlueLight(v) => {
                    let ok = sdk.set_bluelight(v);
                    // 切換藍光等級後，螢幕會連動調整亮度/對比，重新讀取基本參數並保留進階 VCP 狀態
                    let settings = sdk.read_basic_settings(&cached_custom);
                    cached_custom = settings.clone();
                    let _ = evt_tx.send(WorkerEvent::CustomLoaded(settings));
                    let _ = evt_tx.send(WorkerEvent::ActionResult(if ok {
                        format!("藍光等級已設為 {v}")
                    } else {
                        "設定藍光失敗".to_string()
                    }));
                }
                WorkerCmd::SetGamma(v) => {
                    let ok = sdk.set_gamma(v);
                    let _ = evt_tx.send(WorkerEvent::ActionResult(if ok {
                        format!("色差補正/Gamma 已設為 {v}")
                    } else {
                        "設定色差補正失敗".to_string()
                    }));
                }
                WorkerCmd::SetColorTemp(v) => {
                    let ok = sdk.set_colortemp(v);
                    let settings = sdk.read_basic_settings(&cached_custom);
                    cached_custom = settings.clone();
                    let _ = evt_tx.send(WorkerEvent::CustomLoaded(settings));
                    let _ = evt_tx.send(WorkerEvent::ActionResult(if ok {
                        format!("色溫已設為 {v}")
                    } else {
                        "設定色溫失敗".to_string()
                    }));
                }
                WorkerCmd::SetRefreshRate(enabled) => {
                    let ok = sdk.set_refresh_rate(enabled);
                    let _ = evt_tx.send(WorkerEvent::ActionResult(if ok {
                        format!("更新頻率數字已{}", if enabled { "開啟" } else { "關閉" })
                    } else {
                        "設定更新頻率數字失敗".to_string()
                    }));
                }
                WorkerCmd::SetOd(v) => {
                    let ok = sdk.set_od(v);
                    let _ = evt_tx.send(WorkerEvent::ActionResult(if ok {
                        format!("顯示反應速度已設為 {v}")
                    } else {
                        "設定顯示反應速度失敗".to_string()
                    }));
                }
                WorkerCmd::SetAimType(v) => {
                    let ok = sdk.set_aim_type(v);
                    let _ = evt_tx.send(WorkerEvent::ActionResult(if ok {
                        if v == 0 {
                            "瞄準點已關閉".to_string()
                        } else {
                            format!("瞄準點已設為樣式 {v}")
                        }
                    } else {
                        "設定瞄準點失敗".to_string()
                    }));
                }
                WorkerCmd::SetDisplayMode {
                    mode,
                    auto_hdr_device,
                } => {
                    if !sdk.is_initialized() {
                        if let Some(info) = &current_display {
                            let _ = sdk.init(info.index, &info.device_name);
                        }
                    }

                    // 必須在切換模式「之前」先關閉準心，否則在標準/ECO/圖形模式下寫入 AimType 會讓螢幕韌體跳回「使用者」模式
                    if !matches!(mode, DisplayMode::Action | DisplayMode::Racing | DisplayMode::Sports)
                        && sdk.get_aim_type().unwrap_or(0) != 0
                    {
                        let _ = sdk.set_aim_type(0);
                    }

                    let mut hdr_note = String::new();
                    let mut hdr_changed = false;
                    if let Some(dev) = auto_hdr_device {
                        let want_hdr = mode == DisplayMode::Hdr;
                        // 若要開啟 HDR，先切螢幕到 HDR 模式再打開 Windows HDR
                        if want_hdr && sdk.is_initialized() {
                            let _ = sdk.set_display_mode(DisplayMode::Hdr);
                            thread::sleep(Duration::from_millis(300));
                        }
                        match set_windows_hdr(&dev, want_hdr) {
                            Ok(true) => {
                                hdr_changed = true;
                                hdr_note = if want_hdr {
                                    "（已連動開啟 Windows HDR）".to_string()
                                } else {
                                    "（已連動關閉 Windows HDR）".to_string()
                                };
                            }
                            Ok(false) => {}
                            Err(e) => {
                                hdr_note = format!("（Windows HDR 切換失敗: {e}）");
                            }
                        }
                    }

                    // Windows 切換 HDR/SDR 時螢幕會黑畫面重新握手，必須等 init_MCCS 真正成功才可呼叫 DLL，否則會 NULL 指標閃退
                    let mut ok = false;
                    if hdr_changed {
                        if reinit_monitor(sdk, &current_display) {
                            ok = sdk.set_display_mode(mode);
                            if !ok {
                                thread::sleep(Duration::from_millis(400));
                                ok = sdk.set_display_mode(mode);
                            }
                        }
                    } else if sdk.is_initialized() {
                        ok = sdk.set_display_mode(mode);
                    }
                    // 若是開啟 Windows HDR，螢幕本身已由 Windows HDR 訊號強制進入 HDR，即使 DDC/CI 短暫無回應也視為成功
                    if mode == DisplayMode::Hdr && hdr_changed {
                        ok = true;
                    }

                    let (b, m) = query_state(sdk);
                    let _ = evt_tx.send(WorkerEvent::MonitorState {
                        connected: sdk.is_initialized() || (mode == DisplayMode::Hdr && hdr_changed),
                        brightness: b,
                        mode: m.or(if ok { Some(mode) } else { None }),
                        status: if ok {
                            format!("已切換模式 ({mode:?}){hdr_note}")
                        } else {
                            format!("切換模式失敗 ({mode:?}){hdr_note}")
                        },
                    });
                }
                WorkerCmd::ApplyProfile {
                    profile: p,
                    auto_hdr_device,
                } => {
                    if let Some(dev) = auto_hdr_device {
                        if matches!(set_windows_hdr(&dev, false), Ok(true)) {
                            let _ = reinit_monitor(sdk, &current_display);
                        }
                    }
                    if !sdk.is_initialized() {
                        if let Some(info) = &current_display {
                            let _ = sdk.init(info.index, &info.device_name);
                        }
                    }
                    if !sdk.is_initialized() {
                        let _ = evt_tx.send(WorkerEvent::MonitorState {
                            connected: false,
                            brightness: None,
                            mode: None,
                            status: format!("螢幕重新握手中，無法套用「{}」，請稍後再試", p.name),
                        });
                        continue;
                    }
                    let _ = sdk.set_display_mode(DisplayMode::User);
                    let _ = sdk.set_bluelight(p.blue_light);
                    let _ = sdk.set_brightness(p.brightness);
                    let _ = sdk.set_contrast(p.contrast);
                    let _ = sdk.set_gamma(p.gamma);
                    let _ = sdk.set_colortemp(p.color_temp);
                    if p.color_temp == 4 {
                        let _ = sdk.set_rgain(p.r_gain);
                        let _ = sdk.set_ggain(p.g_gain);
                        let _ = sdk.set_bgain(p.b_gain);
                    }
                    let _ = sdk.set_blackboost(p.black_boost);
                    let _ = sdk.set_volume(p.volume);
                    let _ = sdk.set_od(p.od);
                    let _ = sdk.set_aim_type(p.aim_type);
                    let _ = sdk.set_refresh_rate(p.refresh_rate);
                    let _ = sdk.set_vcp(VCP_RGB_BLACK[0], p.r_black);
                    let _ = sdk.set_vcp(VCP_RGB_BLACK[1], p.g_black);
                    let _ = sdk.set_vcp(VCP_RGB_BLACK[2], p.b_black);
                    let _ = sdk.set_vcp(VCP_SHARPNESS, p.sharpness);
                    for (i, &code) in VCP_SAT.iter().enumerate() {
                        let _ = sdk.set_vcp(code, p.sat_6[i]);
                    }
                    for (i, &code) in VCP_HUE.iter().enumerate() {
                        let _ = sdk.set_vcp(code, p.hue_6[i]);
                    }
                    cached_custom.r_black.1 = p.r_black;
                    cached_custom.g_black.1 = p.g_black;
                    cached_custom.b_black.1 = p.b_black;
                    cached_custom.sharpness.0 = p.sharpness;
                    cached_custom.sat_6 = p.sat_6;
                    cached_custom.hue_6 = p.hue_6;
                    let settings = sdk.read_basic_settings(&cached_custom);
                    cached_custom = settings.clone();
                    let b = settings.brightness;
                    let _ = evt_tx.send(WorkerEvent::CustomLoaded(settings));
                    let _ = evt_tx.send(WorkerEvent::MonitorState {
                        connected: true,
                        brightness: Some(b),
                        mode: Some(DisplayMode::User),
                        status: format!("已套用自訂模式「{}」", p.name),
                    });
                }
            }
            ctx.request_repaint();
        }
    });

    (cmd_tx, evt_rx)
}

struct DisplayWidgetApp {
    page: Page,
    displays: Vec<DisplayInfo>,
    selected_idx: usize,
    connected: bool,
    custom: CustomSettings,
    pending_slider: Option<(SliderTarget, u32, Instant)>,
    current_mode: Option<DisplayMode>,
    win_oled: WinOledState,
    oled_hw: OledHwState,
    sys_hw: SystemHwState,
    startup_enabled: bool,
    always_on_top: bool,
    auto_hdr: bool,
    edid_model: String,
    edid_serial: String,
    resolution: String,
    burnin_popup_open: bool,
    burnin_start: Option<Instant>,
    focus_mode: bool,
    tray: Option<TrayManager>,
    window_visible: bool,
    quitting: bool,
    profiles: Vec<CustomProfile>,
    active_profile: Option<usize>,
    new_profile_name: String,
    editing_profile_idx: Option<usize>,
    status: String,
    cmd_tx: Sender<WorkerCmd>,
    evt_rx: Receiver<WorkerEvent>,
}

impl DisplayWidgetApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        setup_fonts(&cc.egui_ctx);
        setup_theme(&cc.egui_ctx);

        let (cmd_tx, evt_rx) = spawn_worker(cc.egui_ctx.clone());
        let displays = enumerate_displays();
        if let Some(first) = displays.first() {
            let _ = cmd_tx.send(WorkerCmd::InitMonitor(first.clone()));
        }
        let win_oled = read_win_oled_state();
        let startup_enabled = get_startup_enabled();
        let auto_hdr = get_auto_hdr();
        let (edid_model, edid_serial, resolution) = if let Some(first) = displays.first() {
            let (m, s) = get_monitor_edid_info(&first.device_name);
            let r = get_display_resolution(&first.device_name);
            (
                m.unwrap_or_else(|| first.friendly_name.clone()),
                s.unwrap_or_else(|| "未知".to_string()),
                r,
            )
        } else {
            ("未知".to_string(), "未知".to_string(), "未知".to_string())
        };

        let tray = TrayManager::new().ok();
        let repaint_ctx_1 = cc.egui_ctx.clone();
        tray_icon::TrayIconEvent::set_event_handler(Some(move |_| {
            repaint_ctx_1.request_repaint();
        }));
        let repaint_ctx_2 = cc.egui_ctx.clone();
        tray_icon::menu::MenuEvent::set_event_handler(Some(move |_| {
            repaint_ctx_2.request_repaint();
        }));

        let start_minimized = std::env::args().any(|a| a == "--minimized");
        if start_minimized {
            cc.egui_ctx
                .send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }

        Self {
            page: Page::Main,
            displays,
            selected_idx: 0,
            connected: false,
            custom: CustomSettings::default(),
            pending_slider: None,
            current_mode: None,
            win_oled,
            oled_hw: OledHwState::default(),
            sys_hw: SystemHwState {
                power_on: true,
                ..Default::default()
            },
            startup_enabled,
            always_on_top: false,
            auto_hdr,
            edid_model,
            edid_serial,
            resolution,
            burnin_popup_open: false,
            burnin_start: None,
            focus_mode: false,
            tray,
            window_visible: !start_minimized,
            quitting: false,
            profiles: load_profiles(),
            active_profile: None,
            new_profile_name: String::new(),
            editing_profile_idx: None,
            status: "初始化中...".to_string(),
            cmd_tx,
            evt_rx,
        }
    }

    fn auto_hdr_device(&self) -> Option<String> {
        if self.auto_hdr {
            self.displays
                .get(self.selected_idx)
                .map(|d| d.device_name.clone())
        } else {
            None
        }
    }

    fn poll_events(&mut self) {
        while let Ok(evt) = self.evt_rx.try_recv() {
            match evt {
                WorkerEvent::SdkLoaded(Ok(msg)) | WorkerEvent::ActionResult(msg) => {
                    self.status = msg;
                }
                WorkerEvent::SdkLoaded(Err(err)) => {
                    self.status = err;
                }
                WorkerEvent::MonitorState {
                    connected,
                    brightness,
                    mode,
                    status,
                } => {
                    self.connected = connected;
                    if let Some(b) = brightness {
                        self.custom.brightness = b;
                    }
                    self.current_mode = mode;
                    self.status = status;
                }
                WorkerEvent::CustomLoaded(settings) => {
                    self.custom = settings;
                }
                WorkerEvent::OledLoaded(hw) => {
                    self.oled_hw = hw;
                }
                WorkerEvent::SystemLoaded { hw, serial } => {
                    self.sys_hw = hw;
                    if let Some(sn) = serial {
                        self.edid_serial = sn;
                    }
                }
            }
        }

        if let Some((target, val, last_change)) = self.pending_slider {
            if last_change.elapsed() >= Duration::from_millis(80) {
                let _ = self.cmd_tx.send(WorkerCmd::SetSlider(target, val));
                self.pending_slider = None;
            }
        }
    }

    fn render_slider(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        label: &str,
        target: SliderTarget,
        range: &mut (u32, u32, u32),
        suffix: &str,
    ) {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(label)
                    .size(14.0)
                    .color(egui::Color32::from_rgb(230, 234, 240)),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new(format!("{}{}", range.1, suffix))
                        .size(14.0)
                        .strong()
                        .color(egui::Color32::WHITE),
                );
            });
        });
        ui.add_space(4.0);
        let resp = ui.add_enabled(
            self.connected,
            egui::Slider::new(&mut range.1, range.0..=range.2).show_value(false),
        );
        if resp.changed() {
            self.pending_slider = Some((target, range.1, Instant::now()));
            ctx.request_repaint_after(Duration::from_millis(85));
        }
        if resp.drag_stopped() {
            let _ = self.cmd_tx.send(WorkerCmd::SetSlider(target, range.1));
            self.pending_slider = None;
        }
        ui.add_space(10.0);
    }

    fn render_radio_group(
        ui: &mut egui::Ui,
        title: &str,
        options: &[(u32, &str)],
        current: &mut u32,
        enabled: bool,
    ) -> Option<u32> {
        let mut changed_to = None;
        ui.label(
            egui::RichText::new(title)
                .size(14.0)
                .color(egui::Color32::from_rgb(230, 234, 240)),
        );
        ui.add_space(6.0);
        ui.add_enabled_ui(enabled, |ui| {
            egui::Grid::new(title)
                .num_columns(3)
                .spacing(egui::vec2(16.0, 6.0))
                .show(ui, |ui| {
                    for (idx, (val, label)) in options.iter().enumerate() {
                        if ui.radio_value(current, *val, *label).changed() {
                            changed_to = Some(*val);
                        }
                        if (idx + 1) % 3 == 0 {
                            ui.end_row();
                        }
                    }
                });
        });
        ui.add_space(12.0);
        changed_to
    }

    fn draw_main_page(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.horizontal(|ui| {
            ui.heading(
                egui::RichText::new("顯示設定")
                    .size(20.0)
                    .strong()
                    .color(egui::Color32::WHITE),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("↻ 重新讀取").clicked() {
                    let _ = self.cmd_tx.send(WorkerCmd::Refresh);
                }
            });
        });

        ui.add_space(10.0);

        egui::ScrollArea::vertical()
            .id_salt("main_page_scroll")
            .max_height(ui.available_height() - 28.0)
            .show(ui, |ui| {
                if !self.displays.is_empty() {
            let current_label = format!(
                "{} ({})",
                self.edid_model,
                self.displays[self.selected_idx].device_name
            );
            let prev_idx = self.selected_idx;
            egui::ComboBox::from_id_salt("monitor_select")
                .width(ui.available_width())
                .selected_text(current_label)
                .show_ui(ui, |ui| {
                    for (idx, d) in self.displays.iter().enumerate() {
                        let (m, _) = get_monitor_edid_info(&d.device_name);
                        let label = format!(
                            "{} ({})",
                            m.unwrap_or_else(|| d.friendly_name.clone()),
                            d.device_name
                        );
                        ui.selectable_value(&mut self.selected_idx, idx, label);
                    }
                });
            if self.selected_idx != prev_idx {
                let d = &self.displays[self.selected_idx];
                let (m, s) = get_monitor_edid_info(&d.device_name);
                self.edid_model = m.unwrap_or_else(|| d.friendly_name.clone());
                self.edid_serial = s.unwrap_or_else(|| "未知".to_string());
                self.resolution = get_display_resolution(&d.device_name);
                let _ = self
                    .cmd_tx
                    .send(WorkerCmd::InitMonitor(d.clone()));
            }
        }

        ui.add_space(12.0);
        ui.separator();
        ui.add_space(10.0);

        let mut b = self.custom.brightness;
        self.render_slider(ui, ctx, "亮度", SliderTarget::Brightness, &mut b, "%");
        self.custom.brightness = b;

        ui.separator();
        ui.add_space(10.0);

        ui.label(
            egui::RichText::new("GameView")
                .size(15.0)
                .color(egui::Color32::from_rgb(230, 234, 240)),
        );
        ui.add_space(10.0);

        let card_w = (ui.available_width() - 16.0) / 3.0;
        let card_h = 84.0;

        egui::Grid::new("gameview_grid")
            .spacing(egui::vec2(8.0, 8.0))
            .show(ui, |ui| {
                for (idx, (mode, label)) in DisplayMode::ALL_GAMEVIEW.iter().enumerate() {
                    let is_active = self.current_mode == Some(*mode) && self.active_profile.is_none();
                    let resp = Self::draw_icon_card(
                        ui,
                        egui::vec2(card_w, card_h),
                        label,
                        is_active,
                        self.connected,
                        |painter, center, color| {
                            icons::draw_gameview_icon(painter, center, *mode, color);
                        },
                    );
                    if resp.clicked() {
                        self.current_mode = Some(*mode);
                        self.active_profile = None;
                        let _ = self.cmd_tx.send(WorkerCmd::SetDisplayMode {
                            mode: *mode,
                            auto_hdr_device: self.auto_hdr_device(),
                        });
                    }

                    if (idx + 1) % 3 == 0 {
                        ui.end_row();
                    }
                }

                // 第 9 格（右下角）：自定義模式二級菜單入口
                let custom_label = if let Some(idx) = self.active_profile {
                    self.profiles
                        .get(idx)
                        .map(|p| p.name.as_str())
                        .unwrap_or("自定義模式")
                } else {
                    "自定義模式"
                };
                let resp = Self::draw_icon_card(
                    ui,
                    egui::vec2(card_w, card_h),
                    custom_label,
                    self.active_profile.is_some(),
                    self.connected,
                    |painter, center, color| {
                        icons::draw_custom_modes_icon(painter, center, color);
                    },
                );
                if resp.clicked() {
                    self.page = Page::Profiles;
                }
                ui.end_row();
            });

        ui.add_space(12.0);

        // "自訂" button matching original layout
        let custom_btn = egui::Button::new(
            egui::RichText::new("自訂")
                .size(15.0)
                .color(egui::Color32::WHITE),
        )
        .fill(egui::Color32::from_rgb(48, 52, 62))
        .rounding(egui::Rounding::same(16.0))
        .min_size(egui::vec2(ui.available_width(), 36.0));

        if ui.add_enabled(self.connected, custom_btn).clicked() {
            self.editing_profile_idx = None;
            self.new_profile_name.clear();
            self.page = Page::Custom;
            let _ = self.cmd_tx.send(WorkerCmd::LoadCustom);
        }
            });
    }

    fn draw_profiles_page(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("< 返回").clicked() {
                self.page = Page::Main;
            }
            ui.add_space(8.0);
            ui.heading(
                egui::RichText::new("自定義模式")
                    .size(18.0)
                    .strong()
                    .color(egui::Color32::WHITE),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("➕ 新增").clicked() {
                    self.editing_profile_idx = None;
                    self.new_profile_name.clear();
                    self.page = Page::Custom;
                    let _ = self.cmd_tx.send(WorkerCmd::LoadCustom);
                }
            });
        });

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(10.0);

        if self.profiles.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                ui.label(
                    egui::RichText::new("尚未建立任何自定義模式")
                        .size(15.0)
                        .color(egui::Color32::from_rgb(160, 166, 178)),
                );
                ui.add_space(12.0);
                if ui.button("➕ 立即新增第一個模式").clicked() {
                    self.editing_profile_idx = None;
                    self.new_profile_name.clear();
                    self.page = Page::Custom;
                    let _ = self.cmd_tx.send(WorkerCmd::LoadCustom);
                }
            });
            return;
        }

        let mut delete_idx = None;
        let mut apply_idx = None;
        let mut edit_idx = None;

        egui::ScrollArea::vertical()
            .id_salt("profiles_submenu_scroll")
            .max_height(ui.available_height() - 28.0)
            .show(ui, |ui| {
                for (idx, prof) in self.profiles.iter().enumerate() {
                    let is_active = self.active_profile == Some(idx);
                    ui.horizontal(|ui| {
                        let fill = if is_active {
                            egui::Color32::from_rgb(28, 102, 224)
                        } else {
                            egui::Color32::from_rgb(48, 52, 62)
                        };
                        let btn = egui::Button::new(
                            egui::RichText::new(&prof.name)
                                .size(14.5)
                                .color(egui::Color32::WHITE),
                        )
                        .fill(fill)
                        .rounding(egui::Rounding::same(10.0))
                        .min_size(egui::vec2(ui.available_width() - 82.0, 38.0));

                        if ui.add_enabled(self.connected, btn).clicked() {
                            apply_idx = Some(idx);
                        }
                        if ui
                            .add(egui::Button::new("✏").min_size(egui::vec2(34.0, 38.0)))
                            .on_hover_text("編輯此模式")
                            .clicked()
                        {
                            edit_idx = Some(idx);
                        }
                        if ui
                            .add(egui::Button::new("🗑").min_size(egui::vec2(34.0, 38.0)))
                            .on_hover_text("刪除此模式")
                            .clicked()
                        {
                            delete_idx = Some(idx);
                        }
                    });
                    ui.add_space(8.0);
                }
            });

        if let Some(idx) = apply_idx {
            self.active_profile = Some(idx);
            let prof = self.profiles[idx].clone();
            let _ = self.cmd_tx.send(WorkerCmd::ApplyProfile {
                profile: prof,
                auto_hdr_device: self.auto_hdr_device(),
            });
        }
        if let Some(idx) = edit_idx {
            let prof = &self.profiles[idx];
            self.editing_profile_idx = Some(idx);
            self.new_profile_name = prof.name.clone();
            self.page = Page::Custom;
            self.active_profile = Some(idx);
            let _ = self.cmd_tx.send(WorkerCmd::ApplyProfile {
                profile: prof.clone(),
                auto_hdr_device: self.auto_hdr_device(),
            });
        }
        if let Some(idx) = delete_idx {
            let removed = self.profiles.remove(idx);
            save_profiles(&self.profiles);
            if self.active_profile == Some(idx) {
                self.active_profile = None;
            }
            self.status = format!("已刪除自定義模式「{}」", removed.name);
        }
    }

    fn draw_custom_page(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.horizontal(|ui| {
            if ui.button("< 返回").clicked() {
                self.page = Page::Main;
            }
            ui.add_space(8.0);
            ui.heading(
                egui::RichText::new("適用於使用者模式")
                    .size(18.0)
                    .strong()
                    .color(egui::Color32::WHITE),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("↻").clicked() {
                    let _ = self.cmd_tx.send(WorkerCmd::LoadCustom);
                }
            });
        });

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(8.0);

        egui::ScrollArea::vertical()
            .max_height(ui.available_height() - 28.0)
            .show(ui, |ui| {
                // 自定義模式命名與儲存區塊
                ui.group(|ui| {
                    ui.label(
                        egui::RichText::new(if self.editing_profile_idx.is_some() {
                            "編輯並儲存自定義模式"
                        } else {
                            "儲存為自定義模式"
                        })
                        .size(14.0)
                        .strong()
                        .color(egui::Color32::WHITE),
                    );
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.new_profile_name)
                                .hint_text("輸入模式名稱（如：夜間寫程式）")
                                .desired_width(ui.available_width() - 92.0),
                        );
                        let save_label = if self.editing_profile_idx.is_some() {
                            "💾 更新"
                        } else {
                            "➕ 新增"
                        };
                        if ui.button(save_label).clicked() {
                            let name = self.new_profile_name.trim().to_string();
                            if name.is_empty() {
                                self.status = "請先輸入模式名稱".to_string();
                            } else {
                                let prof = CustomProfile::from_settings(name.clone(), &self.custom);
                                if let Some(idx) = self.editing_profile_idx {
                                    if idx < self.profiles.len() {
                                        self.profiles[idx] = prof;
                                        self.status = format!("已更新自定義模式「{name}」");
                                    }
                                } else {
                                    self.profiles.push(prof);
                                    self.active_profile = Some(self.profiles.len() - 1);
                                    self.status = format!("已新增自定義模式「{name}」");
                                }
                                save_profiles(&self.profiles);
                                self.editing_profile_idx = None;
                                self.new_profile_name.clear();
                                self.page = Page::Main;
                            }
                        }
                    });
                });
                ui.add_space(10.0);

                if let Some(v) = Self::render_radio_group(
                    ui,
                    "藍光",
                    &[(0, "標準"), (1, "等級 1"), (2, "等級 2"), (3, "等級 3"), (4, "等級 4")],
                    &mut self.custom.blue_light,
                    self.connected,
                ) {
                    let _ = self.cmd_tx.send(WorkerCmd::SetBlueLight(v));
                }

                let mut b = self.custom.brightness;
                self.render_slider(ui, ctx, "亮度", SliderTarget::Brightness, &mut b, "");
                self.custom.brightness = b;

                let mut c = self.custom.contrast;
                self.render_slider(ui, ctx, "對比", SliderTarget::Contrast, &mut c, "");
                self.custom.contrast = c;

                let mut sh = (0, self.custom.sharpness.0, self.custom.sharpness.1.max(1));
                self.render_slider(ui, ctx, "銳利度 / 超級清晰度 (0x87)", SliderTarget::Sharpness, &mut sh, "");
                self.custom.sharpness.0 = sh.1;

                if let Some(v) = Self::render_radio_group(
                    ui,
                    "色差補正 (Gamma)",
                    &[(0, "1.8 / 標準"), (1, "2.0 / 等級 1"), (2, "2.2 / 等級 2"), (3, "2.4"), (4, "2.6")],
                    &mut self.custom.gamma,
                    self.connected,
                ) {
                    let _ = self.cmd_tx.send(WorkerCmd::SetGamma(v));
                }

                if let Some(v) = Self::render_radio_group(
                    ui,
                    "色溫",
                    &[(0, "暖色溫"), (1, "標準"), (2, "冷色溫"), (3, "藍光"), (4, "自訂")],
                    &mut self.custom.color_temp,
                    self.connected,
                ) {
                    let _ = self.cmd_tx.send(WorkerCmd::SetColorTemp(v));
                }

                let mut rg = self.custom.r_gain;
                self.render_slider(ui, ctx, "R 增益 (亮部)", SliderTarget::RGain, &mut rg, "");
                self.custom.r_gain = rg;

                let mut gg = self.custom.g_gain;
                self.render_slider(ui, ctx, "G 增益 (亮部)", SliderTarget::GGain, &mut gg, "");
                self.custom.g_gain = gg;

                let mut bg = self.custom.b_gain;
                self.render_slider(ui, ctx, "B 增益 (亮部)", SliderTarget::BGain, &mut bg, "");
                self.custom.b_gain = bg;

                let mut rb = self.custom.r_black;
                self.render_slider(ui, ctx, "R 暗部黑位 (0x6C)", SliderTarget::RBlack, &mut rb, "");
                self.custom.r_black = rb;

                let mut gb = self.custom.g_black;
                self.render_slider(ui, ctx, "G 暗部黑位 (0x6E)", SliderTarget::GBlack, &mut gb, "");
                self.custom.g_black = gb;

                let mut bb_bias = self.custom.b_black;
                self.render_slider(ui, ctx, "B 暗部黑位 (0x70)", SliderTarget::BBlack, &mut bb_bias, "");
                self.custom.b_black = bb_bias;

                const AXIS_NAMES: [&str; 6] = [
                    "紅 (R)",
                    "黃 (Y)",
                    "綠 (G)",
                    "青 (C)",
                    "藍 (B)",
                    "洋紅 (M)",
                ];

                egui::CollapsingHeader::new(
                    egui::RichText::new("🎨 6 軸獨立飽和度 (0x9B~0xA0)")
                        .size(14.0)
                        .color(egui::Color32::from_rgb(230, 234, 240)),
                )
                .default_open(false)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("獨立調整六色濃淡（預設 50）")
                                .size(12.0)
                                .color(egui::Color32::GRAY),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.add_enabled(self.connected, egui::Button::new("重置為 50")).clicked() {
                                self.custom.sat_6 = [50; 6];
                                let _ = self.cmd_tx.send(WorkerCmd::ResetSat6);
                            }
                        });
                    });
                    ui.add_space(4.0);
                    for i in 0..6 {
                        let mut r = (0, self.custom.sat_6[i], 100);
                        let label = format!("{} 飽和度", AXIS_NAMES[i]);
                        self.render_slider(ui, ctx, &label, SliderTarget::Sat6(i), &mut r, "");
                        self.custom.sat_6[i] = r.1;
                    }
                });
                ui.add_space(8.0);

                egui::CollapsingHeader::new(
                    egui::RichText::new("🌈 6 軸獨立色調 (0x59~0x5E)")
                        .size(14.0)
                        .color(egui::Color32::from_rgb(230, 234, 240)),
                )
                .default_open(false)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("獨立微調六色色相偏移（預設 50）")
                                .size(12.0)
                                .color(egui::Color32::GRAY),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.add_enabled(self.connected, egui::Button::new("重置為 50")).clicked() {
                                self.custom.hue_6 = [50; 6];
                                let _ = self.cmd_tx.send(WorkerCmd::ResetHue6);
                            }
                        });
                    });
                    ui.add_space(4.0);
                    for i in 0..6 {
                        let mut r = (0, self.custom.hue_6[i], 100);
                        let label = format!("{} 色調", AXIS_NAMES[i]);
                        self.render_slider(ui, ctx, &label, SliderTarget::Hue6(i), &mut r, "");
                        self.custom.hue_6[i] = r.1;
                    }
                });
                ui.add_space(10.0);

                let mut bb = (0, self.custom.black_boost.0, self.custom.black_boost.1.max(1));
                self.render_slider(ui, ctx, "黑色增豔", SliderTarget::BlackBoost, &mut bb, "");
                self.custom.black_boost.0 = bb.1;

                let mut vol = (0, self.custom.volume.0, self.custom.volume.1.max(1));
                self.render_slider(ui, ctx, "音量", SliderTarget::Volume, &mut vol, "");
                self.custom.volume.0 = vol.1;

                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("更新頻率數字")
                            .size(14.0)
                            .color(egui::Color32::from_rgb(230, 234, 240)),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if Self::toggle_switch(ui, &mut self.custom.refresh_rate, self.connected) {
                            let _ = self
                                .cmd_tx
                                .send(WorkerCmd::SetRefreshRate(self.custom.refresh_rate));
                        }
                    });
                });
                ui.add_space(12.0);

                if let Some(v) = Self::render_radio_group(
                    ui,
                    "顯示反應速度",
                    &[(0, "關"), (1, "標準"), (2, "極度")],
                    &mut self.custom.od,
                    self.connected,
                ) {
                    let _ = self.cmd_tx.send(WorkerCmd::SetOd(v));
                }

                if let Some(v) = Self::render_radio_group(
                    ui,
                    "瞄準點 (AimPoint)",
                    &[(0, "關"), (1, "樣式 1"), (2, "樣式 2"), (3, "樣式 3")],
                    &mut self.custom.aim_type,
                    self.connected,
                ) {
                    let _ = self.cmd_tx.send(WorkerCmd::SetAimType(v));
                }
            });
    }

    fn draw_oled_page(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading(
                egui::RichText::new("OLED Care")
                    .size(20.0)
                    .strong()
                    .color(egui::Color32::WHITE),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("↻ 重新讀取").clicked() {
                    self.win_oled = read_win_oled_state();
                    let _ = self.cmd_tx.send(WorkerCmd::LoadOled);
                }
            });
        });

        ui.add_space(10.0);
        ui.separator();
        ui.add_space(10.0);

        egui::ScrollArea::vertical()
            .max_height(ui.available_height() - 28.0)
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new("避免影像殘留")
                        .size(15.0)
                        .color(egui::Color32::from_rgb(230, 234, 240)),
                );
                ui.add_space(8.0);

                let card_w = (ui.available_width() - 16.0) / 3.0;
                let card_h = 80.0;

                ui.horizontal(|ui| {
                    if Self::draw_icon_card(
                        ui,
                        egui::vec2(card_w, card_h),
                        "像素重新整理",
                        false,
                        self.connected && self.oled_hw.pixel_refresh_supported,
                        |p, c, col| icons::draw_oled_card_icon(p, c, 0, col),
                    )
                    .clicked()
                    {
                        let _ = self.cmd_tx.send(WorkerCmd::TriggerPixelRefresh);
                    }

                    if Self::draw_icon_card(
                        ui,
                        egui::vec2(card_w, card_h),
                        "消除燒屏",
                        self.burnin_popup_open,
                        true,
                        |p, c, col| icons::draw_oled_card_icon(p, c, 1, col),
                    )
                    .clicked()
                    {
                        self.burnin_popup_open = !self.burnin_popup_open;
                        self.burnin_start = Some(Instant::now());
                        self.status =
                            "已啟動全螢幕消除燒屏色彩循環（按 Esc 或點擊畫面結束）".to_string();
                    }

                    if Self::draw_icon_card(
                        ui,
                        egui::vec2(card_w, card_h),
                        "螢幕保護",
                        self.win_oled.screensaver,
                        true,
                        |p, c, col| icons::draw_oled_card_icon(p, c, 2, col),
                    )
                    .clicked()
                    {
                        self.win_oled.screensaver = !self.win_oled.screensaver;
                        let ok = set_screensaver(self.win_oled.screensaver);
                        self.status = if ok {
                            format!(
                                "Windows 螢幕保護已{}",
                                if self.win_oled.screensaver { "開啟" } else { "關閉" }
                            )
                        } else {
                            "設定螢幕保護失敗".to_string()
                        };
                    }
                });

                ui.add_space(14.0);
                ui.label(
                    egui::RichText::new("主動防護")
                        .size(15.0)
                        .color(egui::Color32::from_rgb(230, 234, 240)),
                );
                ui.add_space(8.0);

                if Self::oled_toggle_row(ui, "深色模式設定", &mut self.win_oled.dark_mode, true) {
                    let ok = set_dark_mode(self.win_oled.dark_mode);
                    self.status = if ok {
                        format!(
                            "Windows 深色模式已{}",
                            if self.win_oled.dark_mode { "開啟" } else { "關閉" }
                        )
                    } else {
                        "設定深色模式失敗".to_string()
                    };
                }

                if Self::oled_toggle_row(ui, "透明效果", &mut self.win_oled.transparency, true) {
                    let ok = set_transparency(self.win_oled.transparency);
                    self.status = if ok {
                        format!(
                            "Windows 透明效果已{}",
                            if self.win_oled.transparency { "開啟" } else { "關閉" }
                        )
                    } else {
                        "設定透明效果失敗".to_string()
                    };
                }

                if Self::oled_toggle_row(ui, "任務欄保護 (自動隱藏)", &mut self.win_oled.taskbar_autohide, true) {
                    set_taskbar_autohide(self.win_oled.taskbar_autohide);
                    self.status = format!(
                        "任務列自動隱藏已{}",
                        if self.win_oled.taskbar_autohide { "開啟" } else { "關閉" }
                    );
                }

                let mut ps = self.oled_hw.pixel_shift.unwrap_or(false);
                if Self::oled_toggle_row(
                    ui,
                    "像素移位",
                    &mut ps,
                    self.connected && self.oled_hw.pixel_shift.is_some(),
                ) {
                    self.oled_hw.pixel_shift = Some(ps);
                    let _ = self.cmd_tx.send(WorkerCmd::SetPixelShift(ps));
                }

                let mut ld = self.oled_hw.logo_detection.unwrap_or(false);
                if Self::oled_toggle_row(
                    ui,
                    "標誌檢測",
                    &mut ld,
                    self.connected && self.oled_hw.logo_detection.is_some(),
                ) {
                    self.oled_hw.logo_detection = Some(ld);
                    let _ = self.cmd_tx.send(WorkerCmd::SetLogoDetection(ld));
                }

                let mut si = self.oled_hw.static_image.unwrap_or(false);
                if Self::oled_toggle_row(
                    ui,
                    "靜態圖像檢測",
                    &mut si,
                    self.connected && self.oled_hw.static_image.is_some(),
                ) {
                    self.oled_hw.static_image = Some(si);
                    let _ = self.cmd_tx.send(WorkerCmd::SetStaticImage(si));
                }

                ui.add_space(12.0);
                ui.label(
                    egui::RichText::new("AI Protection")
                        .size(15.0)
                        .color(egui::Color32::from_rgb(230, 234, 240)),
                );
                ui.add_space(8.0);

                let mut ai = self.oled_hw.ai_protection.unwrap_or(false);
                if Self::oled_toggle_row(
                    ui,
                    "人體存在感測",
                    &mut ai,
                    self.connected && self.oled_hw.ai_protection.is_some(),
                ) {
                    self.oled_hw.ai_protection = Some(ai);
                    let _ = self.cmd_tx.send(WorkerCmd::SetAiProtection(ai));
                }

                if Self::oled_toggle_row(ui, "專注模式", &mut self.focus_mode, true) {
                    self.status = format!(
                        "專注模式已{}",
                        if self.focus_mode { "開啟" } else { "關閉" }
                    );
                }
            });
    }

    fn draw_system_page(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.horizontal(|ui| {
            ui.heading(
                egui::RichText::new("系統設定")
                    .size(20.0)
                    .strong()
                    .color(egui::Color32::WHITE),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("↻ 重新讀取").clicked() {
                    self.startup_enabled = get_startup_enabled();
                    if let Some(d) = self.displays.get(self.selected_idx) {
                        let (m, s) = get_monitor_edid_info(&d.device_name);
                        self.edid_model = m.unwrap_or_else(|| d.friendly_name.clone());
                        self.edid_serial = s.unwrap_or_else(|| "未知".to_string());
                        self.resolution = get_display_resolution(&d.device_name);
                    }
                    let _ = self.cmd_tx.send(WorkerCmd::LoadSystem);
                }
            });
        });

        ui.add_space(10.0);
        ui.separator();
        ui.add_space(10.0);

        egui::ScrollArea::vertical()
            .max_height(ui.available_height() - 28.0)
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new("偏好設定")
                        .size(15.0)
                        .color(egui::Color32::from_rgb(230, 234, 240)),
                );
                ui.add_space(8.0);

                if Self::oled_toggle_row(ui, "電源", &mut self.sys_hw.power_on, self.connected) {
                    let _ = self.cmd_tx.send(WorkerCmd::SetPower(self.sys_hw.power_on));
                }

                let mut pi = self.sys_hw.power_indicator.unwrap_or(false);
                if Self::oled_toggle_row(
                    ui,
                    "電源指示燈",
                    &mut pi,
                    self.connected && self.sys_hw.power_indicator.is_some(),
                ) {
                    self.sys_hw.power_indicator = Some(pi);
                    let _ = self.cmd_tx.send(WorkerCmd::SetPowerIndicator(pi));
                }

                let mut pkl = self.sys_hw.power_key_lock.unwrap_or(false);
                if Self::oled_toggle_row(
                    ui,
                    "電源按鍵鎖定",
                    &mut pkl,
                    self.connected && self.sys_hw.power_key_lock.is_some(),
                ) {
                    self.sys_hw.power_key_lock = Some(pkl);
                    let _ = self.cmd_tx.send(WorkerCmd::SetPowerKeyLock(pkl));
                }

                let mut kl = self.sys_hw.key_lock.unwrap_or(false);
                if Self::oled_toggle_row(
                    ui,
                    "按鍵鎖定",
                    &mut kl,
                    self.connected && self.sys_hw.key_lock.is_some(),
                ) {
                    self.sys_hw.key_lock = Some(kl);
                    let _ = self.cmd_tx.send(WorkerCmd::SetKeyLock(kl));
                }

                if Self::oled_toggle_row(ui, "啟動時執行", &mut self.startup_enabled, true) {
                    let ok = set_startup_enabled(self.startup_enabled);
                    self.status = if ok {
                        format!(
                            "開機自動啟動已{}",
                            if self.startup_enabled { "開啟" } else { "關閉" }
                        )
                    } else {
                        "設定開機自動啟動失敗".to_string()
                    };
                }

                if Self::oled_toggle_row(ui, "最上層顯示", &mut self.always_on_top, true) {
                    let level = if self.always_on_top {
                        egui::WindowLevel::AlwaysOnTop
                    } else {
                        egui::WindowLevel::Normal
                    };
                    ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(level));
                    self.status = format!(
                        "最上層顯示已{}",
                        if self.always_on_top { "開啟" } else { "關閉" }
                    );
                }

                if Self::oled_toggle_row(ui, "自動 HDR (連動 Windows HDR)", &mut self.auto_hdr, true) {
                    let _ = set_auto_hdr(self.auto_hdr);
                    self.status = format!(
                        "自動 HDR 已{}",
                        if self.auto_hdr { "開啟" } else { "關閉" }
                    );
                }

                ui.add_space(4.0);
                let input_supported = self.connected && self.sys_hw.input_source.is_some();
                let mut in_val = self.sys_hw.input_source.unwrap_or(15);
                let in_label = match in_val {
                    15 => "DP (DisplayPort)",
                    17 => "HDMI 1",
                    18 => "HDMI 2",
                    19 => "Type-C",
                    _ => "目前輸入源",
                };
                ui.add_enabled_ui(input_supported, |ui| {
                    ui.label(
                        egui::RichText::new("訊號輸入源切換 (VCP 0x60)")
                            .size(14.0)
                            .color(if input_supported {
                                egui::Color32::from_rgb(230, 234, 240)
                            } else {
                                egui::Color32::from_rgb(110, 115, 125)
                            }),
                    );
                    ui.add_space(4.0);
                    egui::ComboBox::from_id_salt("input_source_select")
                        .width(ui.available_width())
                        .selected_text(in_label)
                        .show_ui(ui, |ui| {
                            let prev = in_val;
                            ui.selectable_value(&mut in_val, 15, "DP (DisplayPort)");
                            ui.selectable_value(&mut in_val, 17, "HDMI 1");
                            ui.selectable_value(&mut in_val, 18, "HDMI 2");
                            if in_val != prev {
                                self.sys_hw.input_source = Some(in_val);
                                let _ = self.cmd_tx.send(WorkerCmd::SetInputSource(in_val));
                            }
                        });
                });
                ui.add_space(8.0);

                ui.add_space(4.0);
                let usb_supported = self.connected && self.sys_hw.usb_port.is_some();
                let mut usb_val = self.sys_hw.usb_port.unwrap_or(0);
                let usb_label = match usb_val {
                    1 => "USB-B",
                    2 => "USB-C",
                    _ => "Auto",
                };
                ui.add_enabled_ui(usb_supported, |ui| {
                    ui.label(
                        egui::RichText::new("USB Port Select")
                            .size(14.0)
                            .color(if usb_supported {
                                egui::Color32::from_rgb(230, 234, 240)
                            } else {
                                egui::Color32::from_rgb(110, 115, 125)
                            }),
                    );
                    ui.add_space(4.0);
                    egui::ComboBox::from_id_salt("usb_port_select")
                        .width(ui.available_width())
                        .selected_text(usb_label)
                        .show_ui(ui, |ui| {
                            let prev = usb_val;
                            ui.selectable_value(&mut usb_val, 0, "Auto");
                            ui.selectable_value(&mut usb_val, 1, "USB-B");
                            ui.selectable_value(&mut usb_val, 2, "USB-C");
                            if usb_val != prev {
                                self.sys_hw.usb_port = Some(usb_val);
                                let _ = self.cmd_tx.send(WorkerCmd::SetUsbPort(usb_val));
                            }
                        });
                });

                ui.add_space(16.0);
                ui.separator();
                ui.add_space(12.0);

                ui.label(
                    egui::RichText::new("資訊")
                        .size(15.0)
                        .color(egui::Color32::from_rgb(230, 234, 240)),
                );
                ui.add_space(8.0);

                Self::info_row(ui, "型號", &self.edid_model);
                Self::info_row(ui, "S/N", &self.edid_serial);
                Self::info_row(ui, "解析度", &self.resolution);
                Self::info_row(ui, "軟體版本", env!("CARGO_PKG_VERSION"));
            });
    }

    fn info_row(ui: &mut egui::Ui, label: &str, value: &str) {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(label)
                    .size(14.0)
                    .color(egui::Color32::from_rgb(180, 186, 196)),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new(value)
                        .size(14.0)
                        .color(egui::Color32::WHITE),
                );
            });
        });
        ui.add_space(6.0);
    }

    fn draw_icon_card(
        ui: &mut egui::Ui,
        size: egui::Vec2,
        label: &str,
        active: bool,
        enabled: bool,
        draw_icon: impl FnOnce(&egui::Painter, egui::Pos2, egui::Color32),
    ) -> egui::Response {
        let sense = if enabled {
            egui::Sense::click()
        } else {
            egui::Sense::hover()
        };
        let (rect, resp) = ui.allocate_exact_size(size, sense);
        if ui.is_rect_visible(rect) {
            let fill = if !enabled {
                egui::Color32::from_rgb(38, 41, 48)
            } else if active {
                egui::Color32::from_rgb(28, 102, 224)
            } else if resp.hovered() {
                egui::Color32::from_rgb(62, 67, 78)
            } else {
                egui::Color32::from_rgb(48, 52, 62)
            };
            let fg = if !enabled {
                egui::Color32::from_rgb(105, 110, 120)
            } else {
                egui::Color32::WHITE
            };
            let painter = ui.painter();
            painter.rect_filled(rect, egui::Rounding::same(10.0), fill);
            if active && enabled {
                painter.rect_stroke(
                    rect,
                    egui::Rounding::same(10.0),
                    egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(90, 155, 255)),
                );
            }
            let icon_center = egui::pos2(rect.center().x, rect.top() + rect.height() * 0.38);
            draw_icon(painter, icon_center, fg);
            painter.text(
                egui::pos2(rect.center().x, rect.bottom() - 16.0),
                egui::Align2::CENTER_CENTER,
                label,
                egui::FontId::proportional(13.5),
                fg,
            );
        }
        resp
    }

    fn toggle_switch(ui: &mut egui::Ui, on: &mut bool, enabled: bool) -> bool {
        let desired_size = egui::vec2(42.0, 22.0);
        let sense = if enabled {
            egui::Sense::click()
        } else {
            egui::Sense::hover()
        };
        let (rect, mut response) = ui.allocate_exact_size(desired_size, sense);
        if response.clicked() && enabled {
            *on = !*on;
            response.mark_changed();
        }
        if ui.is_rect_visible(rect) {
            let how_on = ui.ctx().animate_bool(response.id, *on);
            let bg = if !enabled {
                egui::Color32::from_rgb(42, 45, 52)
            } else if *on {
                egui::Color32::from_rgb(28, 102, 224)
            } else {
                egui::Color32::from_rgb(64, 68, 78)
            };
            let knob = if enabled {
                egui::Color32::WHITE
            } else {
                egui::Color32::from_rgb(95, 100, 110)
            };
            let radius = 0.5 * rect.height();
            ui.painter().rect_filled(rect, radius, bg);
            let circle_x = egui::lerp((rect.left() + radius)..=(rect.right() - radius), how_on);
            ui.painter()
                .circle_filled(egui::pos2(circle_x, rect.center().y), 0.78 * radius, knob);
        }
        response.changed()
    }

    fn oled_toggle_row(ui: &mut egui::Ui, label: &str, value: &mut bool, enabled: bool) -> bool {
        let mut changed = false;
        ui.horizontal(|ui| {
            let color = if enabled {
                egui::Color32::from_rgb(230, 234, 240)
            } else {
                egui::Color32::from_rgb(110, 115, 125)
            };
            ui.label(egui::RichText::new(label).size(14.0).color(color));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                changed = Self::toggle_switch(ui, value, enabled);
            });
        });
        ui.add_space(6.0);
        changed
    }

    fn draw_burnin_viewport(&mut self, ctx: &egui::Context) {
        if !self.burnin_popup_open {
            return;
        }
        let elapsed = self.burnin_start.map(|t| t.elapsed().as_secs()).unwrap_or(0);
        let colors = [
            egui::Color32::RED,
            egui::Color32::GREEN,
            egui::Color32::BLUE,
            egui::Color32::WHITE,
            egui::Color32::BLACK,
        ];
        let color = colors[(elapsed as usize) % colors.len()];
        let mut close_req = false;

        ctx.show_viewport_immediate(
            egui::ViewportId::from_hash_of("oled_burnin_popup"),
            egui::ViewportBuilder::default()
                .with_title("消除燒屏")
                .with_fullscreen(true)
                .with_always_on_top(),
            |ctx, _| {
                if ctx.input(|i| i.key_pressed(egui::Key::Escape) || i.pointer.any_click()) {
                    close_req = true;
                }
                if ctx.input(|i| i.viewport().close_requested()) {
                    close_req = true;
                }
                egui::CentralPanel::default()
                    .frame(egui::Frame::none().fill(color))
                    .show(ctx, |ui| {
                        ui.centered_and_justified(|ui| {
                            ui.label(
                                egui::RichText::new("消除燒屏色彩循環中 — 點擊滑鼠或按 Esc 結束")
                                    .size(18.0)
                                    .color(egui::Color32::GRAY),
                            );
                        });
                    });
                ctx.request_repaint_after(Duration::from_millis(500));
            },
        );

        if close_req {
            self.burnin_popup_open = false;
            self.status = "已結束消除燒屏".to_string();
        }
    }

    fn draw_focus_viewport(&mut self, ctx: &egui::Context) {
        if !self.focus_mode {
            return;
        }
        ctx.show_viewport_immediate(
            egui::ViewportId::from_hash_of("oled_focus_overlay"),
            egui::ViewportBuilder::default()
                .with_title("專注模式")
                .with_fullscreen(true)
                .with_decorations(false)
                .with_transparent(true)
                .with_mouse_passthrough(true)
                .with_always_on_top(),
            |ctx, _| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::none().fill(egui::Color32::from_rgba_unmultiplied(0, 0, 0, 110)))
                    .show(ctx, |_| {});
            },
        );
    }
}

impl eframe::App for DisplayWidgetApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Some(tray) = &self.tray {
            if let Some(action) = tray.poll() {
                match action {
                    TrayAction::ToggleWindow => {
                        self.window_visible = !self.window_visible;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(self.window_visible));
                        if self.window_visible {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                        }
                    }
                    TrayAction::Quit => {
                        self.quitting = true;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
            }
        }

        if ctx.input(|i| i.viewport().close_requested()) && !self.quitting && self.tray.is_some() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            self.window_visible = false;
        }

        self.poll_events();
        self.draw_burnin_viewport(ctx);
        self.draw_focus_viewport(ctx);

        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(egui::Color32::from_rgb(28, 30, 36))
                    .inner_margin(egui::Margin::same(18.0)),
            )
            .show(ctx, |ui| {
                // Top navigation tabs matching main widget bar
                ui.horizontal(|ui| {
                    let is_display = matches!(self.page, Page::Main | Page::Custom | Page::Profiles);
                    if ui
                        .selectable_label(is_display, egui::RichText::new("🖥 顯示設定").size(15.0))
                        .clicked()
                    {
                        self.page = Page::Main;
                    }
                    ui.add_space(8.0);
                    let is_oled = self.page == Page::OledCare;
                    if ui
                        .selectable_label(is_oled, egui::RichText::new("🛡 OLED Care").size(15.0))
                        .clicked()
                    {
                        self.page = Page::OledCare;
                        self.win_oled = read_win_oled_state();
                        let _ = self.cmd_tx.send(WorkerCmd::LoadOled);
                    }
                    ui.add_space(8.0);
                    let is_sys = self.page == Page::SystemSettings;
                    if ui
                        .selectable_label(is_sys, egui::RichText::new("⚙ 系統設定").size(15.0))
                        .clicked()
                    {
                        self.page = Page::SystemSettings;
                        self.startup_enabled = get_startup_enabled();
                        if let Some(d) = self.displays.get(self.selected_idx) {
                            let (m, s) = get_monitor_edid_info(&d.device_name);
                            self.edid_model = m.unwrap_or_else(|| d.friendly_name.clone());
                            self.edid_serial = s.unwrap_or_else(|| "未知".to_string());
                            self.resolution = get_display_resolution(&d.device_name);
                        }
                        let _ = self.cmd_tx.send(WorkerCmd::LoadSystem);
                    }
                });
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);

                match self.page {
                    Page::Main => self.draw_main_page(ui, ctx),
                    Page::Custom => self.draw_custom_page(ui, ctx),
                    Page::Profiles => self.draw_profiles_page(ui),
                    Page::OledCare => self.draw_oled_page(ui),
                    Page::SystemSettings => self.draw_system_page(ui, ctx),
                }

                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    ui.label(
                        egui::RichText::new(&self.status)
                            .size(12.0)
                            .color(egui::Color32::from_rgb(150, 158, 172)),
                    );
                });
            });
    }
}

fn setup_fonts(ctx: &egui::Context) {
    let candidates = [
        r"C:\Windows\Fonts\msjh.ttc",
        r"C:\Windows\Fonts\msyh.ttc",
        r"C:\Windows\Fonts\segoeui.ttf",
    ];
    let mut fonts = egui::FontDefinitions::default();
    for path in candidates {
        if let Ok(bytes) = std::fs::read(path) {
            fonts
                .font_data
                .insert("cjk_font".to_owned(), egui::FontData::from_owned(bytes));
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, "cjk_font".to_owned());
            fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .push("cjk_font".to_owned());
            break;
        }
    }
    ctx.set_fonts(fonts);
}

fn setup_theme(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.visuals = egui::Visuals::dark();
    style.spacing.slider_width = 330.0;
    ctx.set_style(style);
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([380.0, 660.0])
            .with_resizable(false)
            .with_title("Acer Display Widget (Rust)"),
        ..Default::default()
    };

    eframe::run_native(
        "Acer Display Widget (Rust)",
        options,
        Box::new(|cc| Ok(Box::new(DisplayWidgetApp::new(cc)))),
    )
}
