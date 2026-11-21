use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use windows::core::{w, PCSTR, PCWSTR};
use windows::Win32::Devices::Display::{
    DisplayConfigGetDeviceInfo, DisplayConfigSetDeviceInfo, GetDisplayConfigBufferSizes,
    QueryDisplayConfig, DISPLAYCONFIG_DEVICE_INFO_GET_ADVANCED_COLOR_INFO,
    DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME, DISPLAYCONFIG_DEVICE_INFO_HEADER,
    DISPLAYCONFIG_DEVICE_INFO_SET_ADVANCED_COLOR_STATE, DISPLAYCONFIG_DEVICE_INFO_TYPE,
    DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO, DISPLAYCONFIG_MODE_INFO, DISPLAYCONFIG_PATH_INFO,
    DISPLAYCONFIG_SET_ADVANCED_COLOR_STATE, DISPLAYCONFIG_SOURCE_DEVICE_NAME,
    QDC_ONLY_ACTIVE_PATHS,
};
use windows::Win32::Foundation::{ERROR_SUCCESS, HWND, LPARAM, WPARAM};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayDevicesA, EnumDisplaySettingsA, DEVMODEA, DISPLAY_DEVICEA, ENUM_CURRENT_SETTINGS,
};
use windows::Win32::System::Registry::{
    RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY,
    HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WRITE, REG_BINARY, REG_DWORD, REG_SZ,
    REG_VALUE_TYPE,
};
use windows::Win32::UI::Shell::{
    SHAppBarMessage, ShellExecuteW, ABM_GETSTATE, ABM_SETSTATE, ABS_AUTOHIDE, APPBARDATA,
};
use windows::Win32::UI::WindowsAndMessaging::{
    SendMessageTimeoutW, SystemParametersInfoW, HWND_BROADCAST, SMTO_ABORTIFHUNG,
    SPIF_SENDCHANGE, SPIF_UPDATEINIFILE, SPI_GETSCREENSAVEACTIVE, SPI_SETSCREENSAVEACTIVE,
    SW_SHOWNORMAL, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, WM_SETTINGCHANGE,
};

const PERSONALIZE_KEY: PCWSTR = w!(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize");

#[derive(Debug, Clone, Default)]
pub struct WinOledState {
    pub dark_mode: bool,
    pub transparency: bool,
    pub taskbar_autohide: bool,
    pub screensaver: bool,
}

fn read_hkcu_dword(subkey: PCWSTR, value_name: PCWSTR) -> Option<u32> {
    let mut hkey = HKEY::default();
    let res = unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, subkey, 0, KEY_READ, &mut hkey) };
    if res.is_err() {
        return None;
    }
    let mut data = 0u32;
    let mut size = std::mem::size_of::<u32>() as u32;
    let mut ty = REG_VALUE_TYPE::default();
    let q = unsafe {
        RegQueryValueExW(
            hkey,
            value_name,
            None,
            Some(&mut ty),
            Some(&mut data as *mut u32 as *mut u8),
            Some(&mut size),
        )
    };
    unsafe {
        let _ = RegCloseKey(hkey);
    }
    if q.is_ok() {
        Some(data)
    } else {
        None
    }
}

fn write_hkcu_dword(subkey: PCWSTR, value_name: PCWSTR, value: u32) -> bool {
    let mut hkey = HKEY::default();
    let res = unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, subkey, 0, KEY_WRITE, &mut hkey) };
    if res.is_err() {
        return false;
    }
    let bytes = value.to_le_bytes();
    let s = unsafe { RegSetValueExW(hkey, value_name, 0, REG_DWORD, Some(&bytes)) };
    unsafe {
        let _ = RegCloseKey(hkey);
    }
    s.is_ok()
}

pub fn read_win_oled_state() -> WinOledState {
    let apps_light = read_hkcu_dword(PERSONALIZE_KEY, w!("AppsUseLightTheme")).unwrap_or(1);
    let trans = read_hkcu_dword(PERSONALIZE_KEY, w!("EnableTransparency")).unwrap_or(1);

    let mut abd = APPBARDATA {
        cbSize: std::mem::size_of::<APPBARDATA>() as u32,
        ..Default::default()
    };
    let state = unsafe { SHAppBarMessage(ABM_GETSTATE, &mut abd) };
    let taskbar_autohide = (state & (ABS_AUTOHIDE as usize)) != 0;

    let mut ss_active: i32 = 0;
    let _ = unsafe {
        SystemParametersInfoW(
            SPI_GETSCREENSAVEACTIVE,
            0,
            Some(&mut ss_active as *mut i32 as *mut std::ffi::c_void),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    };

    WinOledState {
        dark_mode: apps_light == 0,
        transparency: trans != 0,
        taskbar_autohide,
        screensaver: ss_active != 0,
    }
}

pub fn set_dark_mode(dark: bool) -> bool {
    let val = if dark { 0u32 } else { 1u32 };
    let ok1 = write_hkcu_dword(PERSONALIZE_KEY, w!("AppsUseLightTheme"), val);
    let ok2 = write_hkcu_dword(PERSONALIZE_KEY, w!("SystemUsesLightTheme"), val);
    let param = w!("ImmersiveColorSet");
    unsafe {
        let _ = SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            WPARAM(0),
            LPARAM(param.as_ptr() as isize),
            SMTO_ABORTIFHUNG,
            1000,
            None,
        );
    }
    ok1 || ok2
}

pub fn set_transparency(enabled: bool) -> bool {
    write_hkcu_dword(PERSONALIZE_KEY, w!("EnableTransparency"), u32::from(enabled))
}

pub fn set_taskbar_autohide(enabled: bool) -> bool {
    let mut abd = APPBARDATA {
        cbSize: std::mem::size_of::<APPBARDATA>() as u32,
        hWnd: HWND::default(),
        lParam: LPARAM(if enabled { ABS_AUTOHIDE as isize } else { 0 }),
        ..Default::default()
    };
    unsafe { SHAppBarMessage(ABM_SETSTATE, &mut abd) };
    true
}

pub fn set_screensaver(enabled: bool) -> bool {
    unsafe {
        SystemParametersInfoW(
            SPI_SETSCREENSAVEACTIVE,
            u32::from(enabled),
            None,
            SPIF_UPDATEINIFILE | SPIF_SENDCHANGE,
        )
        .is_ok()
    }
}

const RUN_KEY: PCWSTR = w!(r"Software\Microsoft\Windows\CurrentVersion\Run");
const STARTUP_VALUE: PCWSTR = w!("AcerDisplayRust");

pub fn get_startup_enabled() -> bool {
    let mut hkey = HKEY::default();
    if unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, RUN_KEY, 0, KEY_READ, &mut hkey) }.is_err() {
        return false;
    }
    let mut size = 0u32;
    let res = unsafe { RegQueryValueExW(hkey, STARTUP_VALUE, None, None, None, Some(&mut size)) };
    unsafe {
        let _ = RegCloseKey(hkey);
    }
    res.is_ok() && size > 0
}

pub fn set_startup_enabled(enabled: bool) -> bool {
    let mut hkey = HKEY::default();
    if unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, RUN_KEY, 0, KEY_WRITE, &mut hkey) }.is_err() {
        return false;
    }
    let ok = if enabled {
        if let Ok(exe) = std::env::current_exe() {
            let mut wide: Vec<u16> = format!("\"{}\" --minimized", exe.display())
                .encode_utf16()
                .collect();
            wide.push(0);
            let bytes = unsafe {
                std::slice::from_raw_parts(wide.as_ptr() as *const u8, wide.len() * 2)
            };
            unsafe { RegSetValueExW(hkey, STARTUP_VALUE, 0, REG_SZ, Some(bytes)) }.is_ok()
        } else {
            false
        }
    } else {
        unsafe { RegDeleteValueW(hkey, STARTUP_VALUE) }.is_ok()
    };
    unsafe {
        let _ = RegCloseKey(hkey);
    }
    ok
}

pub fn get_display_resolution(device_name: &str) -> String {
    let Ok(c_name) = CString::new(device_name) else {
        return "未知".to_string();
    };
    let mut dm = DEVMODEA {
        dmSize: std::mem::size_of::<DEVMODEA>() as u16,
        ..Default::default()
    };
    let ok = unsafe {
        EnumDisplaySettingsA(PCSTR(c_name.as_ptr() as *const u8), ENUM_CURRENT_SETTINGS, &mut dm)
    }
    .as_bool();
    if ok {
        format!(
            "{}x{}@{} Hz",
            dm.dmPelsWidth, dm.dmPelsHeight, dm.dmDisplayFrequency
        )
    } else {
        "未知".to_string()
    }
}

pub fn get_monitor_edid_info(device_name: &str) -> (Option<String>, Option<String>) {
    let Ok(c_name) = CString::new(device_name) else {
        return (None, None);
    };
    let mut dd = DISPLAY_DEVICEA {
        cb: std::mem::size_of::<DISPLAY_DEVICEA>() as u32,
        ..Default::default()
    };
    // EDD_GET_DEVICE_INTERFACE_NAME = 1
    let ok = unsafe {
        EnumDisplayDevicesA(PCSTR(c_name.as_ptr() as *const u8), 0, &mut dd, 1)
    }
    .as_bool();
    if !ok {
        return (None, None);
    }
    let dev_id = unsafe { CStr::from_ptr(dd.DeviceID.as_ptr() as *const c_char) }
        .to_string_lossy()
        .into_owned();
    // Format: \\?\DISPLAY#<Model>#<Instance>#{guid}
    let parts: Vec<&str> = dev_id.split('#').collect();
    if parts.len() < 3 {
        return (None, None);
    }
    let reg_path = format!(
        r"SYSTEM\CurrentControlSet\Enum\DISPLAY\{}\{}\Device Parameters",
        parts[1], parts[2]
    );
    let mut wide_path: Vec<u16> = reg_path.encode_utf16().collect();
    wide_path.push(0);

    let mut hkey = HKEY::default();
    if unsafe {
        RegOpenKeyExW(
            HKEY_LOCAL_MACHINE,
            PCWSTR(wide_path.as_ptr()),
            0,
            KEY_READ,
            &mut hkey,
        )
    }
    .is_err()
    {
        return (None, None);
    }

    let mut buf = [0u8; 1024];
    let mut size = buf.len() as u32;
    let mut ty = REG_BINARY;
    let q = unsafe {
        RegQueryValueExW(
            hkey,
            w!("EDID"),
            None,
            Some(&mut ty),
            Some(buf.as_mut_ptr()),
            Some(&mut size),
        )
    };
    unsafe {
        let _ = RegCloseKey(hkey);
    }
    if q.is_err() || size < 128 {
        return (None, None);
    }

    let mut model = None;
    let mut serial = None;
    for offset in [54usize, 72, 90, 108] {
        let block = &buf[offset..offset + 18];
        if block[0] == 0 && block[1] == 0 && block[2] == 0 {
            let text = String::from_utf8_lossy(&block[5..18])
                .trim_matches(|c: char| c.is_control() || c == ' ')
                .to_string();
            if !text.is_empty() {
                if block[3] == 0xFC {
                    model = Some(text);
                } else if block[3] == 0xFF {
                    serial = Some(text);
                }
            }
        }
    }
    (model, serial)
}

const APP_SETTINGS_KEY: PCWSTR = w!(r"Software\AcerDisplayRust");

pub fn get_auto_hdr() -> bool {
    read_hkcu_dword(APP_SETTINGS_KEY, w!("AutoHdr"))
        .map(|v| v != 0)
        .unwrap_or(false)
}

pub fn set_auto_hdr(enabled: bool) -> bool {
    let mut hkey = HKEY::default();
    let res = unsafe {
        windows::Win32::System::Registry::RegCreateKeyW(
            HKEY_CURRENT_USER,
            APP_SETTINGS_KEY,
            &mut hkey,
        )
    };
    if res.is_err() {
        return false;
    }
    let bytes = u32::from(enabled).to_le_bytes();
    let s = unsafe { RegSetValueExW(hkey, w!("AutoHdr"), 0, REG_DWORD, Some(&bytes)) };
    unsafe {
        let _ = RegCloseKey(hkey);
    }
    s.is_ok()
}

pub fn set_windows_hdr(device_name: &str, enable: bool) -> Result<bool, String> {
    let mut path_count = 0u32;
    let mut mode_count = 0u32;
    let err = unsafe {
        GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut path_count, &mut mode_count)
    };
    if err != ERROR_SUCCESS {
        return Err(format!("GetDisplayConfigBufferSizes 失敗: {err:?}"));
    }

    let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); path_count as usize];
    let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); mode_count as usize];
    let err = unsafe {
        QueryDisplayConfig(
            QDC_ONLY_ACTIVE_PATHS,
            &mut path_count,
            paths.as_mut_ptr(),
            &mut mode_count,
            modes.as_mut_ptr(),
            None,
        )
    };
    if err != ERROR_SUCCESS {
        return Err(format!("QueryDisplayConfig 失敗: {err:?}"));
    }

    for path in paths.iter().take(path_count as usize) {
        let mut src_name = DISPLAYCONFIG_SOURCE_DEVICE_NAME {
            header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
                r#type: DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
                size: std::mem::size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>() as u32,
                adapterId: path.sourceInfo.adapterId,
                id: path.sourceInfo.id,
            },
            ..Default::default()
        };
        if unsafe { DisplayConfigGetDeviceInfo(&mut src_name.header) } != 0 {
            continue;
        }
        let end = src_name
            .viewGdiDeviceName
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(src_name.viewGdiDeviceName.len());
        let gdi_name = String::from_utf16_lossy(&src_name.viewGdiDeviceName[..end]);
        if !gdi_name.eq_ignore_ascii_case(device_name) {
            continue;
        }

        // Windows 11 22H2/24H2+: DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO_2 (type = 15)
        // 區分 WCG (自動色彩管理 ACM) 與真正的 HDR (activeColorMode == 2, bit 5 = highDynamicRangeUserEnabled)
        #[repr(C)]
        struct DisplayConfigGetAdvancedColorInfo2 {
            header: DISPLAYCONFIG_DEVICE_INFO_HEADER,
            value: u32,
            color_encoding: i32,
            bits_per_color_channel: u32,
            active_color_mode: i32,
        }

        #[repr(C)]
        struct DisplayConfigSetHdrState {
            header: DISPLAYCONFIG_DEVICE_INFO_HEADER,
            value: u32,
        }

        let mut info2 = DisplayConfigGetAdvancedColorInfo2 {
            header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
                r#type: DISPLAYCONFIG_DEVICE_INFO_TYPE(15),
                size: std::mem::size_of::<DisplayConfigGetAdvancedColorInfo2>() as u32,
                adapterId: path.targetInfo.adapterId,
                id: path.targetInfo.id,
            },
            value: 0,
            color_encoding: 0,
            bits_per_color_channel: 0,
            active_color_mode: 0,
        };

        if unsafe { DisplayConfigGetDeviceInfo(&mut info2.header) } == 0 {
            let hdr_supported = (info2.value & 0x10) != 0 || (info2.value & 0x1) != 0;
            let hdr_enabled = info2.active_color_mode == 2 || (info2.value & 0x20) != 0;
            if !hdr_supported {
                return Err("此螢幕不支援 Windows HDR".to_string());
            }
            if hdr_enabled == enable {
                return Ok(false);
            }

            // DISPLAYCONFIG_DEVICE_INFO_SET_HDR_STATE = 16
            let set_hdr2 = DisplayConfigSetHdrState {
                header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
                    r#type: DISPLAYCONFIG_DEVICE_INFO_TYPE(16),
                    size: std::mem::size_of::<DisplayConfigSetHdrState>() as u32,
                    adapterId: path.targetInfo.adapterId,
                    id: path.targetInfo.id,
                },
                value: u32::from(enable),
            };
            let rc = unsafe { DisplayConfigSetDeviceInfo(&set_hdr2.header) };
            if rc == 0 {
                return Ok(true);
            }
        }

        // Fallback for older Windows 10 builds
        let mut get_hdr = DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO {
            header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
                r#type: DISPLAYCONFIG_DEVICE_INFO_GET_ADVANCED_COLOR_INFO,
                size: std::mem::size_of::<DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO>() as u32,
                adapterId: path.targetInfo.adapterId,
                id: path.targetInfo.id,
            },
            ..Default::default()
        };
        if unsafe { DisplayConfigGetDeviceInfo(&mut get_hdr.header) } != 0 {
            return Err("無法查詢顯示器 HDR 狀態".to_string());
        }
        let val = unsafe { get_hdr.Anonymous.value };
        let supported = (val & 0x1) != 0;
        let currently_enabled = (val & 0x2) != 0;
        if !supported {
            return Err("此螢幕未啟用或系統回報不支援 Windows HDR".to_string());
        }
        if currently_enabled == enable {
            return Ok(false);
        }

        let mut set_hdr = DISPLAYCONFIG_SET_ADVANCED_COLOR_STATE {
            header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
                r#type: DISPLAYCONFIG_DEVICE_INFO_SET_ADVANCED_COLOR_STATE,
                size: std::mem::size_of::<DISPLAYCONFIG_SET_ADVANCED_COLOR_STATE>() as u32,
                adapterId: path.targetInfo.adapterId,
                id: path.targetInfo.id,
            },
            ..Default::default()
        };
        set_hdr.Anonymous.value = u32::from(enable);
        let rc = unsafe { DisplayConfigSetDeviceInfo(&set_hdr.header) };
        if rc != 0 {
            return Err(format!("DisplayConfigSetDeviceInfo 失敗 ({rc})"));
        }
        return Ok(true);
    }

    Err(format!("找不到對應的顯示路徑: {device_name}"))
}

pub fn open_power_sleep_settings() -> bool {
    let rc = unsafe {
        ShellExecuteW(
            HWND::default(),
            w!("open"),
            w!("ms-settings:powersleep"),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    (rc.0 as isize) > 32
}
