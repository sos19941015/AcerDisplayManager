use std::cell::RefCell;
use std::ffi::CStr;
use std::os::raw::c_char;
use std::thread;
use std::time::Duration;
use windows::core::PCSTR;
use windows::Win32::Devices::Display::{
    DestroyPhysicalMonitors, GetCapabilitiesStringLength, GetMonitorBrightness, GetMonitorContrast,
    GetMonitorRedGreenOrBlueGain, GetNumberOfPhysicalMonitorsFromHMONITOR,
    GetPhysicalMonitorsFromHMONITOR, GetVCPFeatureAndVCPFeatureReply, SetMonitorBrightness,
    SetMonitorContrast, SetMonitorRedGreenOrBlueGain, SetVCPFeature, MC_BLUE_GAIN, MC_GAIN_TYPE,
    MC_GREEN_GAIN, MC_RED_GAIN, PHYSICAL_MONITOR,
};
use windows::Win32::Foundation::{BOOL, HANDLE, LPARAM, RECT};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayDevicesA, EnumDisplayMonitors, GetMonitorInfoW, DISPLAY_DEVICEA,
    DISPLAY_DEVICE_ATTACHED_TO_DESKTOP, HDC, HMONITOR, MONITORINFO, MONITORINFOEXW,
};

const BANK_DELAY: Duration = Duration::from_millis(50);

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayMode {
    User = 0,
    Standard = 1,
    Eco = 2,
    Graphic = 3,
    Movie = 4,
    Action = 5, // Game1
    Racing = 6, // Game2
    Sports = 7, // Game3
    AdobeRgb = 8,
    SRgb = 9,
    Rec709 = 10,
    Hdr = 11,
    Reading = 12,
    Darkroom = 13,
    DciP3 = 14,
    CadCam = 15,
}

impl DisplayMode {
    pub const ALL_GAMEVIEW: [(DisplayMode, &'static str); 8] = [
        (DisplayMode::Action, "動作"),
        (DisplayMode::Racing, "競賽"),
        (DisplayMode::Sports, "運動"),
        (DisplayMode::Hdr, "HDR"),
        (DisplayMode::User, "使用者"),
        (DisplayMode::Standard, "標準"),
        (DisplayMode::Eco, "ECO"),
        (DisplayMode::Graphic, "圖形"),
    ];

    pub fn from_u32(val: u32) -> Option<Self> {
        match val {
            0 => Some(Self::User),
            1 => Some(Self::Standard),
            2 => Some(Self::Eco),
            3 => Some(Self::Graphic),
            4 => Some(Self::Movie),
            5 => Some(Self::Action),
            6 => Some(Self::Racing),
            7 => Some(Self::Sports),
            8 => Some(Self::AdobeRgb),
            9 => Some(Self::SRgb),
            10 => Some(Self::Rec709),
            11 => Some(Self::Hdr),
            12 => Some(Self::Reading),
            13 => Some(Self::Darkroom),
            14 => Some(Self::DciP3),
            15 => Some(Self::CadCam),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DisplayInfo {
    pub index: i32,
    pub device_name: String,
    pub friendly_name: String,
}

/// Enumerate active desktop display devices via Win32 `EnumDisplayDevicesA`.
pub fn enumerate_displays() -> Vec<DisplayInfo> {
    let mut result = Vec::new();
    let mut i = 0u32;
    loop {
        let mut dd = DISPLAY_DEVICEA {
            cb: std::mem::size_of::<DISPLAY_DEVICEA>() as u32,
            ..Default::default()
        };
        let ok = unsafe { EnumDisplayDevicesA(PCSTR::null(), i, &mut dd, 0) }.as_bool();
        if !ok {
            break;
        }
        if (dd.StateFlags & DISPLAY_DEVICE_ATTACHED_TO_DESKTOP) != 0 {
            let dev_name = unsafe { CStr::from_ptr(dd.DeviceName.as_ptr() as *const c_char) }
                .to_string_lossy()
                .into_owned();
            let dev_str = unsafe { CStr::from_ptr(dd.DeviceString.as_ptr() as *const c_char) }
                .to_string_lossy()
                .into_owned();
            result.push(DisplayInfo {
                index: result.len() as i32,
                device_name: dev_name,
                friendly_name: dev_str,
            });
        }
        i += 1;
    }
    result
}

pub const VCP_HUE: [u8; 6] = [0x59, 0x5A, 0x5B, 0x5C, 0x5D, 0x5E];
pub const VCP_SAT: [u8; 6] = [0x9B, 0x9C, 0x9D, 0x9E, 0x9F, 0xA0];
pub const VCP_RGB_BLACK: [u8; 3] = [0x6C, 0x6E, 0x70];
pub const VCP_SHARPNESS: u8 = 0x87;

#[derive(Debug, Clone)]
pub struct CustomSettings {
    pub brightness: (u32, u32, u32), // (min, cur, max)
    pub contrast: (u32, u32, u32),
    pub r_gain: (u32, u32, u32),
    pub g_gain: (u32, u32, u32),
    pub b_gain: (u32, u32, u32),
    pub r_black: (u32, u32, u32), // VCP 0x6C
    pub g_black: (u32, u32, u32), // VCP 0x6E
    pub b_black: (u32, u32, u32), // VCP 0x70
    pub sharpness: (u32, u32),    // VCP 0x87 (cur, max)
    pub sat_6: [u32; 6],          // VCP 0x9B..=0xA0 (R, Y, G, C, B, M)
    pub hue_6: [u32; 6],          // VCP 0x59..=0x5E (R, Y, G, C, B, M)
    pub blue_light: u32,
    pub gamma: u32,
    pub color_temp: u32,
    pub black_boost: (u32, u32), // (cur, max)
    pub volume: (u32, u32),      // (cur, max)
    pub refresh_rate: bool,
    pub od: u32,
    pub aim_type: u32,
}

impl Default for CustomSettings {
    fn default() -> Self {
        Self {
            brightness: (0, 50, 100),
            contrast: (0, 50, 100),
            r_gain: (0, 50, 100),
            g_gain: (0, 50, 100),
            b_gain: (0, 50, 100),
            r_black: (0, 50, 100),
            g_black: (0, 50, 100),
            b_black: (0, 50, 100),
            sharpness: (2, 4),
            sat_6: [50; 6],
            hue_6: [50; 6],
            blue_light: 0,
            gamma: 0,
            color_temp: 1,
            black_boost: (0, 10),
            volume: (50, 100),
            refresh_rate: false,
            od: 0,
            aim_type: 0,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct OledHwState {
    pub pixel_refresh_supported: bool,
    pub pixel_shift: Option<bool>,
    pub logo_detection: Option<bool>,
    pub static_image: Option<bool>,
    pub ai_protection: Option<bool>,
}

#[derive(Debug, Clone, Default)]
pub struct SystemHwState {
    pub power_on: bool,
    pub power_indicator: Option<bool>,
    pub power_key_lock: Option<bool>,
    pub key_lock: Option<bool>,
    pub usb_port: Option<u32>,
    pub input_source: Option<u32>,
}

struct EnumMonCtx<'a> {
    target_name: &'a str,
    found: Option<HMONITOR>,
}

unsafe extern "system" fn enum_monitor_proc(
    hmon: HMONITOR,
    _hdc: HDC,
    _rect: *mut RECT,
    lparam: LPARAM,
) -> BOOL {
    let ctx = &mut *(lparam.0 as *mut EnumMonCtx<'_>);
    let mut mi = MONITORINFOEXW {
        monitorInfo: MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFOEXW>() as u32,
            ..Default::default()
        },
        ..Default::default()
    };
    if GetMonitorInfoW(hmon, &mut mi.monitorInfo as *mut MONITORINFO).as_bool() {
        let end = mi
            .szDevice
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(mi.szDevice.len());
        let name = String::from_utf16_lossy(&mi.szDevice[..end]);
        if name.eq_ignore_ascii_case(ctx.target_name) {
            ctx.found = Some(hmon);
            return BOOL(0);
        }
    }
    BOOL(1)
}

fn find_hmonitor(device_name: &str) -> Option<HMONITOR> {
    let mut ctx = EnumMonCtx {
        target_name: device_name,
        found: None,
    };
    unsafe {
        let _ = EnumDisplayMonitors(
            HDC::default(),
            None,
            Some(enum_monitor_proc),
            LPARAM(&mut ctx as *mut EnumMonCtx<'_> as isize),
        );
    }
    ctx.found
}

/// Pure Windows native DDC/CI (`dxva2.dll`) controller — zero Acer DLL dependencies.
pub struct MccsSdk {
    monitors: RefCell<Vec<PHYSICAL_MONITOR>>,
}

impl MccsSdk {
    pub fn new() -> Self {
        Self {
            monitors: RefCell::new(Vec::new()),
        }
    }

    pub fn is_initialized(&self) -> bool {
        !self.monitors.borrow().is_empty()
    }

    fn handle(&self) -> Option<HANDLE> {
        self.monitors
            .borrow()
            .first()
            .map(|m| m.hPhysicalMonitor)
    }

    pub fn init(&self, _monitor_index: i32, device_name: &str) -> bool {
        self.free();
        let Some(hmon) = find_hmonitor(device_name) else {
            return false;
        };
        let mut count = 0u32;
        if unsafe { GetNumberOfPhysicalMonitorsFromHMONITOR(hmon, &mut count) }.is_err()
            || count == 0
        {
            return false;
        }
        let mut mons = vec![PHYSICAL_MONITOR::default(); count as usize];
        if unsafe { GetPhysicalMonitorsFromHMONITOR(hmon, &mut mons) }.is_err() {
            return false;
        }
        *self.monitors.borrow_mut() = mons;
        true
    }

    pub fn free(&self) {
        let mut mons = self.monitors.borrow_mut();
        if !mons.is_empty() {
            let _ = unsafe { DestroyPhysicalMonitors(&mons) };
            mons.clear();
        }
    }

    pub fn get_vcp(&self, code: u8) -> Option<(u32, u32)> {
        let h = self.handle()?;
        let (mut cur, mut max) = (0u32, 0u32);
        let ok = unsafe {
            GetVCPFeatureAndVCPFeatureReply(h, code, None, &mut cur, Some(&mut max))
        };
        if ok != 0 {
            Some((cur, max))
        } else {
            None
        }
    }

    pub fn set_vcp(&self, code: u8, val: u32) -> bool {
        let Some(h) = self.handle() else {
            return false;
        };
        unsafe { SetVCPFeature(h, code, val) != 0 }
    }

    fn get_bank_vcp(&self, select_code: u8, sub: u32, data_code: u8) -> Option<(u32, u32)> {
        if !self.set_vcp(select_code, sub) {
            return None;
        }
        thread::sleep(BANK_DELAY);
        self.get_vcp(data_code)
    }

    fn set_bank_vcp(&self, select_code: u8, sub: u32, data_code: u8, val: u32) -> bool {
        if !self.set_vcp(select_code, sub) {
            return false;
        }
        thread::sleep(BANK_DELAY);
        self.set_vcp(data_code, val)
    }

    fn get_rgb_gain(&self, gain_type: MC_GAIN_TYPE) -> Option<(u32, u32, u32)> {
        let h = self.handle()?;
        let (mut min, mut cur, mut max) = (0u32, 0u32, 0u32);
        if unsafe { GetMonitorRedGreenOrBlueGain(h, gain_type, &mut min, &mut cur, &mut max) } != 0
        {
            Some((min, cur, max.max(min + 1)))
        } else {
            None
        }
    }

    fn set_rgb_gain(&self, gain_type: MC_GAIN_TYPE, value: u32) -> bool {
        let Some(h) = self.handle() else {
            return false;
        };
        unsafe { SetMonitorRedGreenOrBlueGain(h, gain_type, value) != 0 }
    }

    pub fn get_brightness(&self) -> Option<(u32, u32, u32)> {
        let h = self.handle()?;
        let (mut min, mut cur, mut max) = (0u32, 0u32, 0u32);
        if unsafe { GetMonitorBrightness(h, &mut min, &mut cur, &mut max) } != 0 {
            Some((min, cur, max.max(min + 1)))
        } else {
            None
        }
    }

    pub fn set_brightness(&self, value: u32) -> bool {
        let Some(h) = self.handle() else {
            return false;
        };
        unsafe { SetMonitorBrightness(h, value) != 0 }
    }

    pub fn get_contrast(&self) -> Option<(u32, u32, u32)> {
        let h = self.handle()?;
        let (mut min, mut cur, mut max) = (0u32, 0u32, 0u32);
        if unsafe { GetMonitorContrast(h, &mut min, &mut cur, &mut max) } != 0 {
            Some((min, cur, max.max(min + 1)))
        } else {
            None
        }
    }

    pub fn set_contrast(&self, value: u32) -> bool {
        let Some(h) = self.handle() else {
            return false;
        };
        unsafe { SetMonitorContrast(h, value) != 0 }
    }

    pub fn set_rgain(&self, value: u32) -> bool {
        self.set_rgb_gain(MC_RED_GAIN, value)
    }

    pub fn set_ggain(&self, value: u32) -> bool {
        self.set_rgb_gain(MC_GREEN_GAIN, value)
    }

    pub fn set_bgain(&self, value: u32) -> bool {
        self.set_rgb_gain(MC_BLUE_GAIN, value)
    }

    pub fn set_bluelight(&self, value: u32) -> bool {
        self.set_bank_vcp(0xE7, 0, 0xE8, value)
    }

    pub fn set_gamma(&self, value: u32) -> bool {
        self.set_bank_vcp(0xE7, 1, 0xE8, value)
    }

    pub fn set_colortemp(&self, value: u32) -> bool {
        self.set_bank_vcp(0xE7, 2, 0xE8, value)
    }

    pub fn set_blackboost(&self, value: u32) -> bool {
        self.set_vcp(0xE5, value)
    }

    pub fn set_volume(&self, value: u32) -> bool {
        self.set_vcp(0x62, value)
    }

    pub fn set_refresh_rate(&self, enabled: bool) -> bool {
        self.set_bank_vcp(0xE0, 5, 0xE1, u32::from(enabled))
    }

    pub fn set_od(&self, value: u32) -> bool {
        self.set_bank_vcp(0xE0, 4, 0xE1, value)
    }

    pub fn get_aim_type(&self) -> Option<u32> {
        self.get_bank_vcp(0xE0, 6, 0xE1).map(|x| x.0)
    }

    pub fn set_aim_type(&self, value: u32) -> bool {
        self.set_bank_vcp(0xE0, 6, 0xE1, value)
    }

    pub fn trigger_pixel_refresh(&self) -> bool {
        self.set_bank_vcp(0xED, 0, 0xEE, 1)
    }

    pub fn set_pixel_shift(&self, enabled: bool) -> bool {
        self.set_bank_vcp(0xED, 1, 0xEE, u32::from(enabled))
    }

    pub fn set_logo_detection(&self, enabled: bool) -> bool {
        self.set_bank_vcp(0xED, 3, 0xEE, u32::from(enabled))
    }

    pub fn set_static_image(&self, enabled: bool) -> bool {
        self.set_bank_vcp(0xED, 2, 0xEE, u32::from(enabled))
    }

    pub fn set_ai_protection(&self, enabled: bool) -> bool {
        self.set_bank_vcp(0xED, 4, 0xEE, u32::from(enabled))
    }

    pub fn read_oled_hw_state(&self) -> OledHwState {
        if !self.is_initialized() {
            return OledHwState::default();
        }
        OledHwState {
            pixel_refresh_supported: self.get_bank_vcp(0xED, 0, 0xEE).is_some(),
            pixel_shift: self.get_bank_vcp(0xED, 1, 0xEE).map(|x| x.0 != 0),
            logo_detection: self.get_bank_vcp(0xED, 3, 0xEE).map(|x| x.0 != 0),
            static_image: self.get_bank_vcp(0xED, 2, 0xEE).map(|x| x.0 != 0),
            ai_protection: self.get_bank_vcp(0xED, 4, 0xEE).map(|x| x.0 != 0),
        }
    }

    pub fn set_power_on(&self, on: bool) -> bool {
        // VCP 0xD6: 1 = Power On, 5 = Power Off
        self.set_vcp(0xD6, if on { 1 } else { 5 })
    }

    pub fn set_power_indicator(&self, on: bool) -> bool {
        self.set_bank_vcp(0xE0, 2, 0xE1, u32::from(on))
    }

    pub fn set_power_key_lock(&self, locked: bool) -> bool {
        self.set_bank_vcp(0xE0, 1, 0xE1, u32::from(locked))
    }

    pub fn set_key_lock(&self, locked: bool) -> bool {
        self.set_bank_vcp(0xE0, 0, 0xE1, u32::from(locked))
    }

    pub fn set_usb_port(&self, port: u32) -> bool {
        self.set_bank_vcp(0xE7, 3, 0xE8, port)
    }

    pub fn read_system_hw_state(&self) -> SystemHwState {
        let Some(h) = self.handle() else {
            return SystemHwState::default();
        };
        let mut cap_len = 0u32;
        let caps_supported =
            unsafe { GetCapabilitiesStringLength(h, &mut cap_len) != 0 } && cap_len > 1;
        let input_source = self.get_vcp(0x60).map(|x| x.0);
        if !caps_supported {
            return SystemHwState {
                power_on: true,
                power_indicator: None,
                power_key_lock: None,
                key_lock: self.get_bank_vcp(0xE0, 0, 0xE1).map(|x| x.0 != 0),
                usb_port: None,
                input_source,
            };
        }
        SystemHwState {
            power_on: true,
            power_indicator: self.get_bank_vcp(0xE0, 2, 0xE1).map(|x| x.0 != 0),
            power_key_lock: self.get_bank_vcp(0xE0, 1, 0xE1).map(|x| x.0 != 0),
            key_lock: self.get_bank_vcp(0xE0, 0, 0xE1).map(|x| x.0 != 0),
            usb_port: self.get_bank_vcp(0xE7, 3, 0xE8).map(|x| x.0),
            input_source,
        }
    }

    pub fn read_serial_number(&self) -> Option<String> {
        if !self.set_vcp(0xE0, 3) {
            return None;
        }
        thread::sleep(Duration::from_millis(40));
        let mut bytes = Vec::with_capacity(24);
        for read_num in 0..6u32 {
            if !self.set_vcp(0xE1, read_num) {
                return None;
            }
            thread::sleep(Duration::from_millis(40));
            let (cur, _) = self.get_vcp(0xE1)?;
            for b in cur.to_be_bytes() {
                if b.is_ascii_alphanumeric() {
                    bytes.push(b);
                }
            }
        }
        let s = String::from_utf8_lossy(&bytes).into_owned();
        if s.len() >= 10 {
            Some(s)
        } else {
            None
        }
    }

    pub fn read_custom_settings(&self) -> CustomSettings {
        if !self.is_initialized() {
            return CustomSettings::default();
        }
        let mut sat_6 = [50u32; 6];
        for (i, &code) in VCP_SAT.iter().enumerate() {
            if let Some((cur, _)) = self.get_vcp(code) {
                sat_6[i] = cur;
            }
        }
        let mut hue_6 = [50u32; 6];
        for (i, &code) in VCP_HUE.iter().enumerate() {
            if let Some((cur, _)) = self.get_vcp(code) {
                hue_6[i] = cur;
            }
        }
        let r_blk = self.get_vcp(VCP_RGB_BLACK[0]).unwrap_or((50, 100));
        let g_blk = self.get_vcp(VCP_RGB_BLACK[1]).unwrap_or((50, 100));
        let b_blk = self.get_vcp(VCP_RGB_BLACK[2]).unwrap_or((50, 100));
        let sharp = self.get_vcp(VCP_SHARPNESS).unwrap_or((2, 4));
        let sharp_max = sharp.1.max(1);

        let base = CustomSettings {
            r_black: (0, r_blk.0.min(r_blk.1.max(1)), r_blk.1.max(1)),
            g_black: (0, g_blk.0.min(g_blk.1.max(1)), g_blk.1.max(1)),
            b_black: (0, b_blk.0.min(b_blk.1.max(1)), b_blk.1.max(1)),
            sharpness: (sharp.0.min(sharp_max), sharp_max),
            sat_6,
            hue_6,
            ..CustomSettings::default()
        };
        self.read_basic_settings(&base)
    }

    pub fn read_basic_settings(&self, current: &CustomSettings) -> CustomSettings {
        if !self.is_initialized() {
            return current.clone();
        }
        CustomSettings {
            brightness: self.get_brightness().unwrap_or((0, 50, 100)),
            contrast: self.get_contrast().unwrap_or((0, 50, 100)),
            r_gain: self.get_rgb_gain(MC_RED_GAIN).unwrap_or((0, 50, 100)),
            g_gain: self.get_rgb_gain(MC_GREEN_GAIN).unwrap_or((0, 50, 100)),
            b_gain: self.get_rgb_gain(MC_BLUE_GAIN).unwrap_or((0, 50, 100)),
            r_black: current.r_black,
            g_black: current.g_black,
            b_black: current.b_black,
            sharpness: current.sharpness,
            sat_6: current.sat_6,
            hue_6: current.hue_6,
            blue_light: self.get_bank_vcp(0xE7, 0, 0xE8).map(|x| x.0).unwrap_or(0),
            gamma: self.get_bank_vcp(0xE7, 1, 0xE8).map(|x| x.0).unwrap_or(0),
            color_temp: self.get_bank_vcp(0xE7, 2, 0xE8).map(|x| x.0).unwrap_or(1),
            black_boost: self.get_vcp(0xE5).unwrap_or((0, 10)),
            volume: self.get_vcp(0x62).unwrap_or((50, 100)),
            refresh_rate: self
                .get_bank_vcp(0xE0, 5, 0xE1)
                .map(|x| x.0 != 0)
                .unwrap_or(false),
            od: self.get_bank_vcp(0xE0, 4, 0xE1).map(|x| x.0).unwrap_or(0),
            aim_type: self.get_aim_type().unwrap_or(0),
        }
    }

    pub fn get_display_mode(&self) -> Option<u32> {
        self.get_vcp(0xE2).map(|x| x.0)
    }

    pub fn set_display_mode(&self, mode: DisplayMode) -> bool {
        self.set_vcp(0xE2, mode as u32)
    }
}

impl Drop for MccsSdk {
    fn drop(&mut self) {
        self.free();
    }
}
