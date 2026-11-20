use crate::mccs::CustomSettings;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct CustomProfile {
    pub name: String,
    pub brightness: u32,
    pub contrast: u32,
    pub blue_light: u32,
    pub gamma: u32,
    pub color_temp: u32,
    pub r_gain: u32,
    pub g_gain: u32,
    pub b_gain: u32,
    pub black_boost: u32,
    pub volume: u32,
    pub od: u32,
    pub aim_type: u32,
    pub refresh_rate: bool,
    pub r_black: u32,
    pub g_black: u32,
    pub b_black: u32,
    pub sharpness: u32,
    pub sat_6: [u32; 6],
    pub hue_6: [u32; 6],
}

impl CustomProfile {
    pub fn from_settings(name: String, s: &CustomSettings) -> Self {
        Self {
            name,
            brightness: s.brightness.1,
            contrast: s.contrast.1,
            blue_light: s.blue_light,
            gamma: s.gamma,
            color_temp: s.color_temp,
            r_gain: s.r_gain.1,
            g_gain: s.g_gain.1,
            b_gain: s.b_gain.1,
            black_boost: s.black_boost.0,
            volume: s.volume.0,
            od: s.od,
            aim_type: s.aim_type,
            refresh_rate: s.refresh_rate,
            r_black: s.r_black.1,
            g_black: s.g_black.1,
            b_black: s.b_black.1,
            sharpness: s.sharpness.0,
            sat_6: s.sat_6,
            hue_6: s.hue_6,
        }
    }

    fn encode_line(&self) -> String {
        let safe_name = self.name.replace(['|', '\n', '\r'], " ");
        let sat_str = self.sat_6.map(|v| v.to_string()).join(",");
        let hue_str = self.hue_6.map(|v| v.to_string()).join(",");
        format!(
            "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
            safe_name,
            self.brightness,
            self.contrast,
            self.blue_light,
            self.gamma,
            self.color_temp,
            self.r_gain,
            self.g_gain,
            self.b_gain,
            self.black_boost,
            self.volume,
            self.od,
            self.aim_type,
            u8::from(self.refresh_rate),
            self.r_black,
            self.g_black,
            self.b_black,
            self.sharpness,
            sat_str,
            hue_str,
        )
    }

    fn parse_6(s: Option<&&str>) -> [u32; 6] {
        let mut arr = [50u32; 6];
        if let Some(raw) = s {
            for (i, part) in raw.split(',').take(6).enumerate() {
                if let Ok(v) = part.trim().parse() {
                    arr[i] = v;
                }
            }
        }
        arr
    }

    fn decode_line(line: &str) -> Option<Self> {
        let p: Vec<&str> = line.split('|').collect();
        if p.len() < 14 {
            return None;
        }
        let has_vcp_ext = p.len() >= 20;
        Some(Self {
            name: p[0].to_string(),
            brightness: p[1].parse().ok()?,
            contrast: p[2].parse().ok()?,
            blue_light: p[3].parse().ok()?,
            gamma: p[4].parse().ok()?,
            color_temp: p[5].parse().ok()?,
            r_gain: p[6].parse().ok()?,
            g_gain: p[7].parse().ok()?,
            b_gain: p[8].parse().ok()?,
            black_boost: p[9].parse().ok()?,
            volume: p[10].parse().ok()?,
            od: p[11].parse().ok()?,
            aim_type: p[12].parse().ok()?,
            refresh_rate: p[13].trim() == "1",
            r_black: if has_vcp_ext { p[14].parse().unwrap_or(50) } else { 50 },
            g_black: if has_vcp_ext { p[15].parse().unwrap_or(50) } else { 50 },
            b_black: if has_vcp_ext { p[16].parse().unwrap_or(50) } else { 50 },
            sharpness: if has_vcp_ext { p[17].parse().unwrap_or(2) } else { 2 },
            sat_6: if has_vcp_ext { Self::parse_6(p.get(18)) } else { [50; 6] },
            hue_6: if has_vcp_ext { Self::parse_6(p.get(19)) } else { [50; 6] },
        })
    }
}

fn profiles_path() -> PathBuf {
    if let Ok(appdata) = std::env::var("APPDATA") {
        let dir = PathBuf::from(appdata).join("AcerDisplayRust");
        let _ = fs::create_dir_all(&dir);
        return dir.join("profiles.txt");
    }
    PathBuf::from("profiles.txt")
}

pub fn load_profiles() -> Vec<CustomProfile> {
    let path = profiles_path();
    let Ok(content) = fs::read_to_string(path) else {
        return Vec::new();
    };
    content
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(CustomProfile::decode_line)
        .collect()
}

pub fn save_profiles(profiles: &[CustomProfile]) {
    let path = profiles_path();
    let mut out = String::new();
    for p in profiles {
        out.push_str(&p.encode_line());
        out.push('\n');
    }
    let _ = fs::write(path, out);
}
