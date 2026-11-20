# AcerDisplayManager

輕量化、原生 64 位元 Rust GUI 工具，用以取代肥大的 .NET Framework 4.8 WPF 版 **Acer Display Widget**。  
**100% 原生呼叫 Windows `dxva2.dll`（DDC/CI）**，完全不依賴任何 Acer 原廠 DLL，編譯後為**單一 `.exe` 檔案即可開箱即用**。

A lightweight, native 64-bit Rust GUI replacement for Acer Display Widget on Windows. Uses Windows native `dxva2.dll` (DDC/CI) directly with **zero proprietary Acer DLL dependencies**.

## 功能特色

- **零外部依賴**：直接透過 Windows 系統內建 `dxva2.dll` (`GetVCPFeatureAndVCPFeatureReply` / `SetVCPFeature`) 與螢幕韌體通訊，不需安裝官方 Acer Display Widget，也不需任何第三方 DLL。
- **GameView 顯示模式切換**：支援 `使用者`、`標準`、`ECO`、`圖形`、`動作`、`競賽`、`運動`、`HDR` 與自定義模式管理。
- **完整硬體 MCCS / VCP 色彩與畫面控制**：
  - 亮度、對比度、藍光濾鏡、Gamma、色溫（含自訂 RGB 增益）、黑色增豔、音量
  - 顯示反應速度 (OD)、更新頻率顯示、瞄準點樣式
  - 銳利度 (`0x87`)、RGB 暗部黑位 (`0x6C/0x6E/0x70`)、6 軸飽和度 (`0x9B~0xA0`)、6 軸色調 (`0x59~0x5E`)
- **自定義模式設定檔**：支援新增、編輯、套用與刪除，儲存在 `%APPDATA%\AcerDisplayRust\profiles.txt`。
- **自動 HDR 連動**：支援 Windows 11 24H2 (`DISPLAYCONFIG_SET_HDR_STATE`)，切到 HDR 模式時自動開啟 Windows HDR，切回 SDR 模式時自動關閉，並於握手完成後自動重連螢幕。
- **OLED Care**：整合 Windows 深色模式、透明效果、自動隱藏工作列、螢幕保護程式、全螢幕殘影修復與專注模式，以及硬體 OLED 防護項目。
- **系統設定與常駐系統匣**：訊號輸入源切換 (`VCP 0x60`)、EDID 型號／序號讀取、開機自動最小化啟動、關閉按鈕縮小至系統匣。

## 編譯與執行

需求：Windows 10/11 (x64)、Rust Toolchain (`cargo`)，且螢幕 OSD 需開啟 **DDC/CI**。

```powershell
# 開發模式執行
cargo run

# 編譯最佳化 Release 單一執行檔（產出於 target\release\acer-display-rust.exe）
cargo build --release
```

## 技術文件

- [Acer 螢幕 DDC/CI (VCP) 通訊協定說明 (ACER_VCP_PROTOCOL.md)](./ACER_VCP_PROTOCOL.md) — 完整整理標準 VCP 與 Acer `0xE0/0xE1`、`0xE7/0xE8`、`0xED/0xEE` 分頁切換協定及實測韌體細節。

## 免責聲明

本專案為社群開源工具，與 Acer Inc. 無任何官方關聯。所有 Acer 商標均歸其各自擁有者所有。
