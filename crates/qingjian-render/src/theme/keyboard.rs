//! 键盘主题：键帽配色、字号、间距、圆角、总高。
//!
//! 与候选窗的 [`Palette`](super::Palette) 同源取色（都对着系统语义色），将来主题文件落地时
//! 两者是同一个文件的两节。所有数值单位是**点**。

use crate::color::Color;
use crate::theme::FontSpec;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KeyboardTheme {
    /// 键盘底色（键帽之间的缝）。
    pub background: Color,

    /// 字母键的键帽。
    pub key: Color,

    /// 按键被按住时的键帽。
    pub key_pressed: Color,

    /// 功能键（Shift / 退格 / 中英 / 逗号）的键帽。
    pub key_function: Color,

    /// 主键（空格 / 回车）的键帽。
    pub key_primary: Color,

    /// 键帽上的文字与图标。
    pub label: Color,

    /// 大写的 Shift 锁定态用的强调色。
    pub accent: Color,

    /// 键帽上的字。
    pub font: FontSpec,

    /// 笔画加深的 gamma——与候选窗用同一个值，两处的字重才一致。
    pub text_gamma: f32,

    /// 键与键之间的缝（点）。
    pub gap: f32,

    /// 键帽圆角（点）。
    pub radius: f32,

    /// 键盘总高（点）。行高按行数均分。
    pub height: f32,
}

impl KeyboardTheme {
    pub const fn light() -> Self {
        Self {
            background: Color::rgb(209, 211, 217),
            key: Color::rgb(255, 255, 255),
            key_pressed: Color::rgb(178, 181, 189),
            key_function: Color::rgb(174, 178, 187),
            key_primary: Color::rgb(174, 178, 187),
            label: Color::gray(0, 216),
            accent: Color::rgb(0, 122, 255),
            font: FontSpec::new(17.0, 22.0),
            text_gamma: 0.85,
            gap: 6.0,
            radius: 5.0,
            height: 208.0,
        }
    }

    pub const fn dark() -> Self {
        Self {
            background: Color::rgb(30, 30, 32),
            key: Color::rgb(107, 107, 110),
            key_pressed: Color::rgb(140, 140, 144),
            key_function: Color::rgb(70, 70, 73),
            key_primary: Color::rgb(70, 70, 73),
            label: Color::gray(255, 230),
            accent: Color::rgb(10, 132, 255),
            font: FontSpec::new(17.0, 22.0),
            text_gamma: 0.75,
            gap: 6.0,
            radius: 5.0,
            height: 208.0,
        }
    }

    /// 某个键帽的底色。
    pub const fn key_color(&self, style: crate::keyboard::KeyStyle, pressed: bool) -> Color {
        if pressed {
            return self.key_pressed;
        }
        match style {
            crate::keyboard::KeyStyle::Letter => self.key,
            crate::keyboard::KeyStyle::Function => self.key_function,
            crate::keyboard::KeyStyle::Primary => self.key_primary,
        }
    }
}
