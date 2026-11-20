# AcerDisplayManager

輕量化、原生 64 位元 Rust GUI 工具，用以取代肥大的 .NET Framework 4.8 WPF 版 **Acer Display Widget**。  
A lightweight, native 64-bit Rust GUI replacement for Acer Display Widget on Windows.

## 功能特色

- **GameView 顯示模式切換**：支援 `使用者`、`標準`、`ECO`、`圖形`、`動作`、`賽車`、`運動`、`HDR` 與自定義模式管理。
- **完整硬體 MCCS / VCP 色彩與畫面控制**：
  - 亮度、對比度、藍光濾鏡、Gamma、色溫（含自訂 RGB 增益）、黑色增豔、音量
  - 顯示反應速度 (OD)、更新頻率顯示、瞄準點樣式
  - 銳利度 (`0x87`)、RGB 暗部黑位 (`0x6C/0x6E/0x70`)、6 軸飽和度 (`0x9B~0xA0`)、6 軸色調 (`0x59~0x5E`)
- **自定義模式設定檔**：支援新增、編輯、套用與刪除，儲存在 `%APPDATA%\AcerDisplayRust\profiles.txt`。
- **自動 HDR 連動**：支援 Windows 11 24H2 (`DISPLAYCONFIG_SET_HDR_STATE`)，切到 HDR 模式時自動開啟 Windows HDR，切回 SDR 模式時自動關閉，並於握手完成後自動重連螢幕。
- **OLED Care**：整合 Windows 深色模式、透明效果、自動隱藏工作列、螢幕保護程式、全螢幕殘影修復與專注模式，以及硬體 OLED 防護項目。
- **系統設定與常駐系統匣**：訊號輸入源切換 (`VCP 0x60`)、EDID 型號／序號讀取、開機自動最小化啟動、關閉按鈕縮小至系統匣。

## 關於 Acer 原廠 DLL（重要）

本專案**僅包含開源 Rust 原始碼，不包含任何 Acer 專有二進位檔或 DLL**。

程式在執行期只會動態載入單一 DLL：**`Monitor_MCCS_API_x64.dll`**（透過 `libloading`）。  
請從您已安裝的官方 **Acer Display Widget** 取得此檔案，程式會依以下順序自動尋找：

1. 與 `acer-display-rust.exe` 相同的資料夾
2. 目前工作目錄或上一層目錄
3. `C:\Program Files (x86)\Acer\Acer Display Widget\Monitor_MCCS_API_x64.dll`
4. `C:\Program Files\Acer\Acer Display Widget\Monitor_MCCS_API_x64.dll`

若您的電腦已安裝官方 Acer Display Widget 於預設路徑，通常不需手動複製即可直接啟動；否則請將 `Monitor_MCCS_API_x64.dll` 複製到本程式執行檔同目錄即可。

## 編譯與執行

需求：Windows 10/11 (x64)、Rust Toolchain (`cargo`)

```powershell
# 開發模式執行
cargo run

# 編譯最佳化 Release 版本（產出於 target\release\acer-display-rust.exe）
cargo build --release
```

## 免責聲明

本專案為社群開源工具，與 Acer Inc. 無任何官方關聯。所有 Acer 商標與原廠 DLL 版權均歸其各自擁有者所有。
