//! 配置文件 `[keyboard]` 分节：键盘的手感项。
//!
//! **只有安卓用**——电脑上没马达。平台专属的配置项有先例（`[general]` 里
//! `english_full_width_punctuation` 注释写着「只有 Windows 用」），配置格式仍是全平台同一份。

use serde::{Deserialize, Serialize};

/// 自定义震动时长的下限（毫秒）。
pub const MIN_VIBRATION_MS: u32 = 1;

/// 自定义震动时长的上限（毫秒）。再长就不像「按了一下键」，更像来电话了。
pub const MAX_VIBRATION_MS: u32 = 50;

/// 自定义震动时长的缺省值（毫秒）。与运动传感器时代之前写死的那个数一致，
/// 选了「自定义」不会突然换一只手感。
pub const DEFAULT_VIBRATION_MS: u32 = 20;

/// 按键震动的感觉。
///
/// 除了 [`Self::Custom`]，其余几档都交给系统的**预制触感**（`VibrationEffect.createPredefined`）：
/// 厂商针对自家马达调过，而自己编一个「通电多少毫秒」在每台机器上表现都不一样
/// （K2 写死的 20 ms 就是当时「试出来的起点」）。四档按系统定的概念能量排：
/// `Tick`（轻）< `Click`（中间值，官方建议的起点）< `Heavy`（重）< `Double`（最高）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VibrationStyle {
    /// 不震。
    Off,

    /// 轻微。
    Tick,

    /// 清脆。缺省：官方建议的基准档，介于轻与重之间。
    #[default]
    Click,

    /// 低沉。
    Heavy,

    /// 双击。
    Double,

    /// 自定义时长，见 [`KeyboardConfig::vibration_ms`]。
    Custom,
}

impl VibrationStyle {
    /// 全部取值，设置界面按这个顺序列出（从轻到重，`Custom` 收尾）。
    pub const ALL: [Self; 6] = [
        Self::Off,
        Self::Tick,
        Self::Click,
        Self::Heavy,
        Self::Double,
        Self::Custom,
    ];

    /// 配置文件里的写法。
    pub fn key(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Tick => "tick",
            Self::Click => "click",
            Self::Heavy => "heavy",
            Self::Double => "double",
            Self::Custom => "custom",
        }
    }

    /// 界面上的名字。
    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "关",
            Self::Tick => "轻微",
            Self::Click => "清脆",
            Self::Heavy => "低沉",
            Self::Double => "双击",
            Self::Custom => "自定义",
        }
    }
}

/// `[keyboard]` 分节。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct KeyboardConfig {
    /// 按键震动的感觉。
    pub vibration: VibrationStyle,

    /// 自定义震动时长（毫秒，1–50）。**只在 `vibration = "custom"` 时用**——
    /// 其余几档的时长是系统按马达定的，不该暴露给用户。
    pub vibration_ms: u32,
}

impl Default for KeyboardConfig {
    fn default() -> Self {
        Self {
            vibration: VibrationStyle::default(),
            vibration_ms: DEFAULT_VIBRATION_MS,
        }
    }
}

impl KeyboardConfig {
    /// 夹到合法范围的震动时长。
    pub fn vibration_ms(&self) -> u32 {
        self.vibration_ms.clamp(MIN_VIBRATION_MS, MAX_VIBRATION_MS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vibration_styles_round_trip_and_cover_every_variant() {
        for style in VibrationStyle::ALL {
            let text = format!("vibration = \"{}\"\n", style.key());
            let config: KeyboardConfig = toml::from_str(&text).unwrap();
            assert_eq!(config.vibration, style);
        }
    }

    #[test]
    fn default_is_click_and_missing_key_falls_back() {
        assert_eq!(KeyboardConfig::default().vibration, VibrationStyle::Click);
        assert_eq!(KeyboardConfig::default().vibration_ms(), 20);
        // 整个键都没写时才走缺省
        let config: KeyboardConfig = toml::from_str("vibration_ms = 30\n").unwrap();
        assert_eq!(config.vibration, VibrationStyle::Click);
        // 写了个不认识的名字要报错，不静默吞掉用户的笔误（与 theme / log_level 同一个规矩）
        assert!(toml::from_str::<KeyboardConfig>("vibration = \"buzz\"\n").is_err());
    }

    #[test]
    fn vibration_ms_is_clamped() {
        let too_short = KeyboardConfig {
            vibration_ms: 0,
            ..KeyboardConfig::default()
        };
        assert_eq!(too_short.vibration_ms(), MIN_VIBRATION_MS);
        let too_long = KeyboardConfig {
            vibration_ms: 500,
            ..KeyboardConfig::default()
        };
        assert_eq!(too_long.vibration_ms(), MAX_VIBRATION_MS);
    }
}
