# Acer 螢幕 DDC/CI (VCP) 通訊協定說明

本文件整理本專案透過 Windows 原生 `dxva2.dll`（`GetVCPFeatureAndVCPFeatureReply` / `SetVCPFeature` / `GetMonitor*` / `SetMonitor*`）與 Acer 螢幕韌體通訊所使用的完整 **VESA MCCS / Acer 專屬 VCP 代碼與協定細節**。

---

## 1. 標準 VESA MCCS 與直接讀寫 VCP

可直接透過 `dxva2.dll` 單次讀寫的功能：

| 功能 | API / VCP Code | 數值說明 |
| --- | --- | --- |
| 亮度 (Brightness) | `GetMonitorBrightness` / `SetMonitorBrightness` (`0x10`) | `0 ~ 100` |
| 對比度 (Contrast) | `GetMonitorContrast` / `SetMonitorContrast` (`0x12`) | `0 ~ 100` |
| R / G / B 亮部增益 (Gain) | `Get/SetMonitorRedGreenOrBlueGain` (`0x16` / `0x18` / `0x1A`) | `0 ~ 100`（通常僅於色溫＝使用者模式生效） |
| 6 軸色調 (6-Axis Hue) | `0x59` (R), `0x5A` (Y), `0x5B` (G), `0x5C` (C), `0x5D` (B), `0x5E` (M) | `0 ~ 100`（預設 `50`） |
| 訊號輸入源 (Input Source) | `0x60` | `15` = DP1、`16` = DP2、`17` = HDMI1、`18` = HDMI2、`27` = USB-C |
| 喇叭音量 (Speaker Volume) | `0x62` | `0 ~ 100` |
| RGB 暗部黑位 (Black Level) | `0x6C` (R), `0x6E` (G), `0x70` (B) | `0 ~ 100`（預設 `50`） |
| 銳利度 (Sharpness) | `0x87` | 依機種而定（例如 `XV272U RV` 為 `0 ~ 4`，預設 `2`） |
| 6 軸飽和度 (6-Axis Saturation) | `0x9B` (R), `0x9C` (Y), `0x9D` (G), `0x9E` (C), `0x9F` (B), `0xA0` (M) | `0 ~ 100`（預設 `50`） |
| 電源模式 (Power Mode) | `0xD6` | `1` = 開啟 (On)、`5` = 關閉 (Off) |
| 顯示模式 (Display Mode) | `0xE2` | 詳見下方「顯示模式對照表」 |
| 支援的顯示模式位元遮罩 | `0xE3` | 唯讀 Bitmask（各 bit 對應 `0xE2` 模式支援度） |
| 黑色增豔 (Black Boost) | `0xE5` | `0 ~ 10` |

### 顯示模式 (`VCP 0xE2`) 數值對照表

| 數值 | 模式名稱 | 說明 |
| --- | --- | --- |
| `0` | User（使用者） | 自訂模式 |
| `1` | Standard（標準） | 標準模式 |
| `2` | ECO | 節能模式 |
| `3` | Graphic（圖形） | 圖形模式 |
| `4` | Movie（電影） | 部分機種支援 |
| `5` | Action（動作 / Game 1） | 遊戲模式 1 |
| `6` | Racing（競賽 / Game 2） | 遊戲模式 2 |
| `7` | Sports（運動 / Game 3） | 遊戲模式 3 |
| `8` | AdobeRGB | 部分機種支援 |
| `9` | sRGB | 部分機種支援 |
| `10` | Rec.709 | 部分機種支援 |
| `11` | HDR | HDR 模式 |
| `12` | Reading（閱讀） | 部分機種支援 |
| `13` | Darkroom（暗房） | 部分機種支援 |
| `14` | DCI-P3 | 部分機種支援 |
| `15` | CAD/CAM | 部分機種支援 |

---

## 2. Acer Bank-Select（分頁切換）VCP 協定

Acer 螢幕將大量專屬設定收在三組「選擇暫存器 + 資料暫存器」的兩段式 VCP 協定中。

### 通用讀寫流程
1. **選擇子項目**：呼叫 `SetVCPFeature(hMonitor, select_code, sub_id)`
2. **等待韌體切換**：延遲 **`50 ms`**
3. **讀取或寫入資料**：
   - 讀取：`GetVCPFeatureAndVCPFeatureReply(hMonitor, data_code, ...)`
   - 寫入：`SetVCPFeature(hMonitor, data_code, value)`

---

### Bank A：`0xE7`（選擇）→ `0xE8`（資料）

| `sub_id` (`0xE7`) | 功能 | 數值說明 (`0xE8`) |
| --- | --- | --- |
| `0` | 藍光濾鏡 (Blue Light) | `0` = 關閉、`1~4` = 等級 1~4（依機種） |
| `1` | Gamma | `0` = 2.2、`1` = 1.8、`2` = 2.0、`3` = 2.4、`4` = 2.6（依機種） |
| `2` | 色溫 (Color Temp) | `0` = 暖色、`1` = 標準、`2` = 冷色、`3` = 藍光、`4` = 使用者自訂 |
| `3` | USB 埠選擇 (USB Port Select) | 依機種 KVM / 上行埠設定 |

---

### Bank B：`0xE0`（選擇）→ `0xE1`（資料）

| `sub_id` (`0xE0`) | 功能 | 數值說明 (`0xE1`) |
| --- | --- | --- |
| `0` | OSD 按鍵鎖定 (Key Lock) | `0` = 解鎖、`1` = 鎖定 |
| `1` | 電源鍵鎖定 (Power Key Lock) | `0` = 解鎖、`1` = 鎖定 |
| `2` | 電源指示燈 (Power Indicator) | `0` = 關閉、`1` = 開啟 |
| `3` | 螢幕序號讀取 (Serial Number) | 特殊多段讀取流程，見下方說明 |
| `4` | 顯示反應速度 (Over Drive / OD) | `0` = 關閉、`1` = 標準、`2` = 極速 |
| `5` | 更新頻率顯示 (Refresh Rate OSD) | `0` = 關閉、`1` = 開啟 |
| `6` | 瞄準點樣式 (Aim Point Type) | `0` = 關閉、`1~3` = 樣式 1~3 |

#### 螢幕序號 (`0xE0 = 3`) 讀取流程
1. `SetVCPFeature(h, 0xE0, 3)` → 延遲 `40 ms`
2. 迴圈 `i = 0..6`：
   - `SetVCPFeature(h, 0xE1, i)` → 延遲 `40 ms`
   - `GetVCPFeatureAndVCPFeatureReply(h, 0xE1, ...)` 取得 32-bit 數值 `cur`
   - 將 `cur` 轉為 Big-Endian 4 bytes，濾出 ASCII 英數字元拼接即為完整序號

---

### Bank C：`0xED`（選擇）→ `0xEE`（資料）— OLED 保護

| `sub_id` (`0xED`) | 功能 | 數值說明 (`0xEE`) |
| --- | --- | --- |
| `0` | 影像殘留重新整理 (Image Retention / Pixel Refresh) | 寫入 `1` 觸發 |
| `1` | 像素移位 (Pixel Centering / Pixel Shift) | `0` = 關閉、`1` = 開啟 |
| `2` | 靜態圖像防護 (Static Image Upkeep) | `0` = 關閉、`1` = 開啟 |
| `3` | 邊緣／標誌檢測 (Edge Protection / Logo Detection) | `0` = 關閉、`1` = 開啟 |
| `4` | AI 人體存在防護 (AI Protection) | `0` = 關閉、`1` = 開啟 |

---

## 3. 實測韌體注意事項（以 `Acer XV272U RV` 為例）

1. **切換模式前需先關閉瞄準點**：若在切換至 `Standard / ECO / Graphic` 模式**之後**才寫入 `AimType = 0`，螢幕韌體會自動跳回 `User` 模式；必須在送出 `0xE2` 切換模式**之前**先關閉瞄準點。
2. **部分 VCP 寫入後讀回延遲／快取現象**：銳利度 (`0x87`)、RGB 暗部黑位 (`0x6C/0x6E/0x70`)、6 軸飽和度 (`0x9B~0xA0`) 與 6 軸色調 (`0x59~0x5E`) 寫入後畫面會立即生效，但短時間內讀回可能仍回傳預設值，因此前端程式應保留使用者設定值避免被覆蓋。
3. **Windows HDR 切換握手期**：透過 `DisplayConfigSetDeviceInfo` 切換 Windows HDR ↔ SDR 時，螢幕會短暫黑畫面並重建實體螢幕控制代碼 (`HMONITOR` / `PHYSICAL_MONITOR`)。此時必須呼叫 `DestroyPhysicalMonitors` 釋放舊 Handle，等待約 1 秒後重試重新取得新 Handle，再發送後續 DDC/CI 指令。
4. **Capabilities String 不支援**：部分 Acer 機種（如 `XV272U RV`）的 `GetCapabilitiesStringLength` 會回傳 `0`，但直接對上述 VCP 代碼讀寫仍然完全有效。
