use libloading::Library;
use std::cell::Cell;
use std::ffi::CStr;
use std::os::raw::c_char;
use std::path::{Path, PathBuf};
use windows::core::PCSTR;
use windows::Win32::Graphics::Gdi::{
    EnumDisplayDevicesA, DISPLAY_DEVICEA, DISPLAY_DEVICE_ATTACHED_TO_DESKTOP,
};

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

type FnInitMccs = unsafe extern "C" fn(i32, *const u16, i32, i32) -> bool;
type FnFreePhysicalMonitor = unsafe extern "C" fn();
type FnGetRange = unsafe extern "C" fn(&mut u32, &mut u32, &mut u32) -> bool;
type FnGetPair = unsafe extern "C" fn(&mut u32, &mut u32) -> bool;
type FnSetU32 = unsafe extern "C" fn(u32) -> bool;
type FnGetVcp = unsafe extern "C" fn(u8, &mut u32, &mut u32) -> bool;
type FnSetVcp = unsafe extern "C" fn(u8, u32) -> bool;
type FnGetSn = unsafe extern "C" fn(&mut u32, &mut u32, i32, i32) -> bool;
type FnGetVcpString = unsafe extern "C" fn() -> *const c_char;

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

pub struct MccsSdk {
    _lib: Library,
    initialized: Cell<bool>,
    init_mccs: FnInitMccs,
    free_monitor: FnFreePhysicalMonitor,
    get_vcp_string: FnGetVcpString,
    get_vcp_code: FnGetVcp,
    set_vcp_code: FnSetVcp,
    get_brightness: FnGetRange,
    set_brightness: FnSetU32,
    get_contrast: FnGetRange,
    set_contrast: FnSetU32,
    get_rgain: FnGetRange,
    set_rgain: FnSetU32,
    get_ggain: FnGetRange,
    set_ggain: FnSetU32,
    get_bgain: FnGetRange,
    set_bgain: FnSetU32,
    get_bluelight: FnGetPair,
    set_bluelight: FnSetU32,
    get_gamma: FnGetPair,
    set_gamma: FnSetU32,
    get_colortemp: FnGetPair,
    set_colortemp: FnSetU32,
    get_blackboost: FnGetPair,
    set_blackboost: FnSetU32,
    get_volume: FnGetPair,
    set_volume: FnSetU32,
    get_refreshrate: FnGetPair,
    set_refreshrate: FnSetU32,
    get_od: FnGetPair,
    set_od: FnSetU32,
    get_aim_type: FnGetPair,
    set_aim_type: FnSetU32,
    get_pixel_refresh: FnGetPair,
    set_pixel_refresh: FnSetU32,
    get_pixel_shift: FnGetPair,
    set_pixel_shift: FnSetU32,
    get_logo_detection: FnGetPair,
    set_logo_detection: FnSetU32,
    get_static_image: FnGetPair,
    set_static_image: FnSetU32,
    get_ai_protection: FnGetPair,
    set_ai_protection: FnSetU32,
    set_power_mode: FnSetU32,
    get_power_indicator: FnGetPair,
    set_power_indicator: FnSetU32,
    get_power_key: FnGetPair,
    set_power_key: FnSetU32,
    get_key_lock: FnGetPair,
    set_key_lock: FnSetU32,
    get_usb_port: FnGetPair,
    set_usb_port: FnSetU32,
    get_sn: FnGetSn,
    get_display_mode: FnGetPair,
    set_display_mode: FnSetU32,
}

impl MccsSdk {
    pub fn find_and_load() -> Result<(Self, PathBuf), String> {
        let dll_name = "Monitor_MCCS_API_x64.dll";
        let mut candidates = vec![
            PathBuf::from(dll_name),
            PathBuf::from("..").join(dll_name),
            PathBuf::from(r"C:\Program Files (x86)\Acer\Acer Display Widget").join(dll_name),
            PathBuf::from(r"C:\Program Files\Acer\Acer Display Widget").join(dll_name),
        ];
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                candidates.insert(0, dir.join(dll_name));
            }
        }

        let found = candidates
            .into_iter()
            .find(|p| p.exists())
            .ok_or_else(|| {
                format!(
                    "找不到 {dll_name}，請從官方 Acer Display Widget 安裝目錄複製此 DLL 到執行檔同目錄"
                )
            })?;

        unsafe { Self::load(&found) }
            .map(|sdk| (sdk, found))
            .map_err(|e| format!("載入 {dll_name} 失敗: {e}"))
    }

    unsafe fn load(dll_path: &Path) -> Result<Self, libloading::Error> {
        let lib = Library::new(dll_path)?;
        Ok(Self {
            init_mccs: *lib.get::<FnInitMccs>(b"init_MCCS\0")?,
            free_monitor: *lib.get::<FnFreePhysicalMonitor>(b"FreePhysicalMonitor\0")?,
            get_vcp_string: *lib.get::<FnGetVcpString>(b"Monitor_Get_VCP_String\0")?,
            get_vcp_code: *lib.get::<FnGetVcp>(b"Monitor_Get_VCP_code\0")?,
            set_vcp_code: *lib.get::<FnSetVcp>(b"Monitor_SET_VCP_code\0")?,
            get_brightness: *lib.get::<FnGetRange>(b"Monitor_GET_Brightness\0")?,
            set_brightness: *lib.get::<FnSetU32>(b"Monitor_SET_Brightness\0")?,
            get_contrast: *lib.get::<FnGetRange>(b"Monitor_GET_Contrast\0")?,
            set_contrast: *lib.get::<FnSetU32>(b"Monitor_SET_Contrast\0")?,
            get_rgain: *lib.get::<FnGetRange>(b"Monitor_GET_Rgain\0")?,
            set_rgain: *lib.get::<FnSetU32>(b"Monitor_SET_Rgain\0")?,
            get_ggain: *lib.get::<FnGetRange>(b"Monitor_GET_Ggain\0")?,
            set_ggain: *lib.get::<FnSetU32>(b"Monitor_SET_Ggain\0")?,
            get_bgain: *lib.get::<FnGetRange>(b"Monitor_GET_Bgain\0")?,
            set_bgain: *lib.get::<FnSetU32>(b"Monitor_SET_Bgain\0")?,
            get_bluelight: *lib.get::<FnGetPair>(b"Monitor_GET_BlueLight\0")?,
            set_bluelight: *lib.get::<FnSetU32>(b"Monitor_Set_BlueLight\0")?,
            get_gamma: *lib.get::<FnGetPair>(b"Monitor_GET_Gamma\0")?,
            set_gamma: *lib.get::<FnSetU32>(b"Monitor_Set_Gamma\0")?,
            get_colortemp: *lib.get::<FnGetPair>(b"Monitor_GET_ColorTemp\0")?,
            set_colortemp: *lib.get::<FnSetU32>(b"Monitor_Set_ColorTemp\0")?,
            get_blackboost: *lib.get::<FnGetPair>(b"Monitor_GET_BlackBoost\0")?,
            set_blackboost: *lib.get::<FnSetU32>(b"Monitor_Set_BlackBoost\0")?,
            get_volume: *lib.get::<FnGetPair>(b"Monitor_GET_SPK_Volume\0")?,
            set_volume: *lib.get::<FnSetU32>(b"Monitor_SET_SPK_Volume\0")?,
            get_refreshrate: *lib.get::<FnGetPair>(b"Monitor_GET_RefreshRate\0")?,
            set_refreshrate: *lib.get::<FnSetU32>(b"Monitor_Set_RefreshRate\0")?,
            get_od: *lib.get::<FnGetPair>(b"Monitor_GET_OD\0")?,
            set_od: *lib.get::<FnSetU32>(b"Monitor_Set_OD\0")?,
            get_aim_type: *lib.get::<FnGetPair>(b"Monitor_GET_AimType\0")?,
            set_aim_type: *lib.get::<FnSetU32>(b"Monitor_Set_AimType\0")?,
            get_pixel_refresh: *lib.get::<FnGetPair>(b"Monitor_GET_ImageRetentionRefresh\0")?,
            set_pixel_refresh: *lib.get::<FnSetU32>(b"Monitor_Set_ImageRetentionRefresh\0")?,
            get_pixel_shift: *lib.get::<FnGetPair>(b"Monitor_GET_PixelCentering\0")?,
            set_pixel_shift: *lib.get::<FnSetU32>(b"Monitor_Set_PixelCentering\0")?,
            get_logo_detection: *lib.get::<FnGetPair>(b"Monitor_GET_EdgeProtection\0")?,
            set_logo_detection: *lib.get::<FnSetU32>(b"Monitor_Set_EdgeProtection\0")?,
            get_static_image: *lib.get::<FnGetPair>(b"Monitor_GET_StaticImageUpkeep\0")?,
            set_static_image: *lib.get::<FnSetU32>(b"Monitor_Set_StaticImageUpkeep\0")?,
            get_ai_protection: *lib.get::<FnGetPair>(b"Monitor_GET_AIProtection\0")?,
            set_ai_protection: *lib.get::<FnSetU32>(b"Monitor_Set_AIProtection\0")?,
            set_power_mode: *lib.get::<FnSetU32>(b"Monitor_Set_PowerModeControl\0")?,
            get_power_indicator: *lib.get::<FnGetPair>(b"Monitor_GET_PowerIndicator\0")?,
            set_power_indicator: *lib.get::<FnSetU32>(b"Monitor_Set_PowerIndicator\0")?,
            get_power_key: *lib.get::<FnGetPair>(b"Monitor_GET_PowerKey\0")?,
            set_power_key: *lib.get::<FnSetU32>(b"Monitor_Set_PowerKey\0")?,
            get_key_lock: *lib.get::<FnGetPair>(b"Monitor_GET_KeyLock\0")?,
            set_key_lock: *lib.get::<FnSetU32>(b"Monitor_Set_KeyLock\0")?,
            get_usb_port: *lib.get::<FnGetPair>(b"Monitor_Get_USBPortSelect\0")?,
            set_usb_port: *lib.get::<FnSetU32>(b"Monitor_Set_USBPortSelect\0")?,
            get_sn: *lib.get::<FnGetSn>(b"Monitor_Get_SN\0")?,
            get_display_mode: *lib.get::<FnGetPair>(b"Monitor_GET_DisplayMode\0")?,
            set_display_mode: *lib.get::<FnSetU32>(b"Monitor_Set_DisplayMode\0")?,
            initialized: Cell::new(false),
            _lib: lib,
        })
    }

    pub fn is_initialized(&self) -> bool {
        self.initialized.get()
    }

    pub fn init(&self, monitor_index: i32, device_name: &str) -> bool {
        self.free();
        let wide_name: Vec<u16> = device_name.encode_utf16().chain(std::iter::once(0)).collect();
        let ok = unsafe { (self.init_mccs)(monitor_index, wide_name.as_ptr(), 0, 0) };
        self.initialized.set(ok);
        ok
    }

    pub fn free(&self) {
        if self.initialized.replace(false) {
            unsafe { (self.free_monitor)() };
        }
    }

    fn read_range(&self, f: FnGetRange) -> Option<(u32, u32, u32)> {
        if !self.initialized.get() {
            return None;
        }
        let (mut min, mut cur, mut max) = (0, 0, 0);
        if unsafe { f(&mut min, &mut cur, &mut max) } {
            Some((min, cur, max.max(min + 1)))
        } else {
            None
        }
    }

    fn read_pair(&self, f: FnGetPair) -> Option<(u32, u32)> {
        if !self.initialized.get() {
            return None;
        }
        let (mut cur, mut max) = (0, 0);
        if unsafe { f(&mut cur, &mut max) } {
            Some((cur, max))
        } else {
            None
        }
    }

    pub fn get_brightness(&self) -> Option<(u32, u32, u32)> {
        self.read_range(self.get_brightness)
    }

    pub fn set_brightness(&self, value: u32) -> bool {
        self.initialized.get() && unsafe { (self.set_brightness)(value) }
    }

    pub fn set_contrast(&self, value: u32) -> bool {
        self.initialized.get() && unsafe { (self.set_contrast)(value) }
    }

    pub fn set_rgain(&self, value: u32) -> bool {
        self.initialized.get() && unsafe { (self.set_rgain)(value) }
    }

    pub fn set_ggain(&self, value: u32) -> bool {
        self.initialized.get() && unsafe { (self.set_ggain)(value) }
    }

    pub fn set_bgain(&self, value: u32) -> bool {
        self.initialized.get() && unsafe { (self.set_bgain)(value) }
    }

    pub fn set_bluelight(&self, value: u32) -> bool {
        self.initialized.get() && unsafe { (self.set_bluelight)(value) }
    }

    pub fn set_gamma(&self, value: u32) -> bool {
        self.initialized.get() && unsafe { (self.set_gamma)(value) }
    }

    pub fn set_colortemp(&self, value: u32) -> bool {
        self.initialized.get() && unsafe { (self.set_colortemp)(value) }
    }

    pub fn set_blackboost(&self, value: u32) -> bool {
        self.initialized.get() && unsafe { (self.set_blackboost)(value) }
    }

    pub fn set_volume(&self, value: u32) -> bool {
        self.initialized.get() && unsafe { (self.set_volume)(value) }
    }

    pub fn set_refresh_rate(&self, enabled: bool) -> bool {
        self.initialized.get() && unsafe { (self.set_refreshrate)(u32::from(enabled)) }
    }

    pub fn set_od(&self, value: u32) -> bool {
        self.initialized.get() && unsafe { (self.set_od)(value) }
    }

    pub fn get_aim_type(&self) -> Option<u32> {
        self.read_pair(self.get_aim_type).map(|x| x.0)
    }

    pub fn set_aim_type(&self, value: u32) -> bool {
        self.initialized.get() && unsafe { (self.set_aim_type)(value) }
    }

    pub fn trigger_pixel_refresh(&self) -> bool {
        self.initialized.get() && unsafe { (self.set_pixel_refresh)(1) }
    }

    pub fn set_pixel_shift(&self, enabled: bool) -> bool {
        self.initialized.get() && unsafe { (self.set_pixel_shift)(u32::from(enabled)) }
    }

    pub fn set_logo_detection(&self, enabled: bool) -> bool {
        self.initialized.get() && unsafe { (self.set_logo_detection)(u32::from(enabled)) }
    }

    pub fn set_static_image(&self, enabled: bool) -> bool {
        self.initialized.get() && unsafe { (self.set_static_image)(u32::from(enabled)) }
    }

    pub fn set_ai_protection(&self, enabled: bool) -> bool {
        self.initialized.get() && unsafe { (self.set_ai_protection)(u32::from(enabled)) }
    }

    pub fn read_oled_hw_state(&self) -> OledHwState {
        OledHwState {
            pixel_refresh_supported: self.read_pair(self.get_pixel_refresh).is_some(),
            pixel_shift: self.read_pair(self.get_pixel_shift).map(|x| x.0 != 0),
            logo_detection: self.read_pair(self.get_logo_detection).map(|x| x.0 != 0),
            static_image: self.read_pair(self.get_static_image).map(|x| x.0 != 0),
            ai_protection: self.read_pair(self.get_ai_protection).map(|x| x.0 != 0),
        }
    }

    pub fn set_power_on(&self, on: bool) -> bool {
        // 1 = Monitor_PowerOn, 5 = Monitor_PowerOff
        self.initialized.get() && unsafe { (self.set_power_mode)(if on { 1 } else { 5 }) }
    }

    pub fn set_power_indicator(&self, on: bool) -> bool {
        self.initialized.get() && unsafe { (self.set_power_indicator)(u32::from(on)) }
    }

    pub fn set_power_key_lock(&self, locked: bool) -> bool {
        self.initialized.get() && unsafe { (self.set_power_key)(u32::from(locked)) }
    }

    pub fn set_key_lock(&self, locked: bool) -> bool {
        self.initialized.get() && unsafe { (self.set_key_lock)(u32::from(locked)) }
    }

    pub fn set_usb_port(&self, port: u32) -> bool {
        self.initialized.get() && unsafe { (self.set_usb_port)(port) }
    }

    pub fn get_vcp(&self, code: u8) -> Option<(u32, u32)> {
        if !self.initialized.get() {
            return None;
        }
        let (mut cur, mut max) = (0, 0);
        if unsafe { (self.get_vcp_code)(code, &mut cur, &mut max) } {
            Some((cur, max))
        } else {
            None
        }
    }

    pub fn set_vcp(&self, code: u8, val: u32) -> bool {
        self.initialized.get() && unsafe { (self.set_vcp_code)(code, val) }
    }

    pub fn read_system_hw_state(&self) -> SystemHwState {
        if !self.initialized.get() {
            return SystemHwState::default();
        }
        let vcp_str = unsafe {
            let ptr = (self.get_vcp_string)();
            if ptr.is_null() {
                "Not Support".to_string()
            } else {
                CStr::from_ptr(ptr).to_string_lossy().into_owned()
            }
        };
        let input_source = self.get_vcp(0x60).map(|x| x.0);
        if vcp_str.contains("Not Support") {
            return SystemHwState {
                power_on: true,
                power_indicator: None,
                power_key_lock: None,
                key_lock: self.read_pair(self.get_key_lock).map(|x| x.0 != 0),
                usb_port: None,
                input_source,
            };
        }
        SystemHwState {
            power_on: true,
            power_indicator: self.read_pair(self.get_power_indicator).map(|x| x.0 != 0),
            power_key_lock: self.read_pair(self.get_power_key).map(|x| x.0 != 0),
            key_lock: self.read_pair(self.get_key_lock).map(|x| x.0 != 0),
            usb_port: self.read_pair(self.get_usb_port).map(|x| x.0),
            input_source,
        }
    }

    pub fn read_serial_number(&self) -> Option<String> {
        if !self.initialized.get() {
            return None;
        }
        let mut bytes = Vec::with_capacity(24);
        for read_num in 0..6 {
            let (mut cur, mut max) = (0u32, 0u32);
            if !unsafe { (self.get_sn)(&mut cur, &mut max, read_num, 40) } {
                return None;
            }
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
        if !self.initialized.get() {
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
        if !self.initialized.get() {
            return current.clone();
        }
        CustomSettings {
            brightness: self.get_brightness().unwrap_or((0, 50, 100)),
            contrast: self.read_range(self.get_contrast).unwrap_or((0, 50, 100)),
            r_gain: self.read_range(self.get_rgain).unwrap_or((0, 50, 100)),
            g_gain: self.read_range(self.get_ggain).unwrap_or((0, 50, 100)),
            b_gain: self.read_range(self.get_bgain).unwrap_or((0, 50, 100)),
            r_black: current.r_black,
            g_black: current.g_black,
            b_black: current.b_black,
            sharpness: current.sharpness,
            sat_6: current.sat_6,
            hue_6: current.hue_6,
            blue_light: self.read_pair(self.get_bluelight).map(|x| x.0).unwrap_or(0),
            gamma: self.read_pair(self.get_gamma).map(|x| x.0).unwrap_or(0),
            color_temp: self.read_pair(self.get_colortemp).map(|x| x.0).unwrap_or(1),
            black_boost: self.read_pair(self.get_blackboost).unwrap_or((0, 10)),
            volume: self.read_pair(self.get_volume).unwrap_or((50, 100)),
            refresh_rate: self.read_pair(self.get_refreshrate)
                .map(|x| x.0 != 0)
                .unwrap_or(false),
            od: self.read_pair(self.get_od).map(|x| x.0).unwrap_or(0),
            aim_type: self.read_pair(self.get_aim_type).map(|x| x.0).unwrap_or(0),
        }
    }

    pub fn get_display_mode(&self) -> Option<u32> {
        self.read_pair(self.get_display_mode).map(|x| x.0)
    }

    pub fn set_display_mode(&self, mode: DisplayMode) -> bool {
        self.initialized.get() && unsafe { (self.set_display_mode)(mode as u32) }
    }
}

impl Drop for MccsSdk {
    fn drop(&mut self) {
        self.free();
    }
}
