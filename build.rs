use std::env;
use std::fs;
use std::path::PathBuf;

fn pad4(buf: &mut Vec<u8>) {
    while buf.len() % 4 != 0 {
        buf.push(0);
    }
}

fn push_res_entry(out: &mut Vec<u8>, res_type: u16, res_id: u16, data: &[u8]) {
    let data_size = data.len() as u32;
    let header_size: u32 = 32;
    out.extend_from_slice(&data_size.to_le_bytes());
    out.extend_from_slice(&header_size.to_le_bytes());
    // Type: ordinal (0xFFFF, res_type)
    out.extend_from_slice(&0xFFFFu16.to_le_bytes());
    out.extend_from_slice(&res_type.to_le_bytes());
    // Name: ordinal (0xFFFF, res_id)
    out.extend_from_slice(&0xFFFFu16.to_le_bytes());
    out.extend_from_slice(&res_id.to_le_bytes());
    // DataVersion (4), MemoryFlags (2), LanguageId (2), Version (4), Characteristics (4)
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0x0030u16.to_le_bytes()); // MOVEABLE | PURE
    out.extend_from_slice(&0x0409u16.to_le_bytes()); // en-US
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(data);
    pad4(out);
}

fn main() {
    println!("cargo:rerun-if-changed=assets/app.ico");
    println!("cargo:rerun-if-changed=build.rs");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let ico = fs::read("assets/app.ico").expect("assets/app.ico must exist");
    assert!(ico.len() >= 6, "invalid ico header");
    let count = u16::from_le_bytes([ico[4], ico[5]]) as usize;
    assert!(ico.len() >= 6 + count * 16, "truncated ico directory");

    let mut res = Vec::new();
    // Standard 32-byte empty Win32 .res header
    res.extend_from_slice(&[
        0x00, 0x00, 0x00, 0x00, 0x20, 0x00, 0x00, 0x00,
        0xFF, 0xFF, 0x00, 0x00, 0xFF, 0xFF, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ]);

    let mut group_icon = Vec::with_capacity(6 + count * 14);
    group_icon.extend_from_slice(&ico[0..6]); // idReserved, idType=1, idCount

    for i in 0..count {
        let entry = &ico[6 + i * 16..6 + (i + 1) * 16];
        let bytes_in_res = u32::from_le_bytes([entry[8], entry[9], entry[10], entry[11]]) as usize;
        let image_offset = u32::from_le_bytes([entry[12], entry[13], entry[14], entry[15]]) as usize;
        let img_data = &ico[image_offset..image_offset + bytes_in_res];
        let icon_id = (i + 1) as u16;

        // RT_ICON = 3
        push_res_entry(&mut res, 3, icon_id, img_data);

        // GRPICONDIRENTRY (14 bytes: first 12 bytes of ICONDIRENTRY + 2-byte nID)
        group_icon.extend_from_slice(&entry[0..12]);
        group_icon.extend_from_slice(&icon_id.to_le_bytes());
    }

    // RT_GROUP_ICON = 14, resource ID = 1 (default application icon)
    push_res_entry(&mut res, 14, 1, &group_icon);

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let res_path = out_dir.join("app_icon.res");
    fs::write(&res_path, &res).expect("write app_icon.res");

    println!("cargo:rustc-link-arg-bins={}", res_path.display());
}
