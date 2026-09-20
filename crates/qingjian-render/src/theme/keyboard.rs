//! 键盘主题：键帽配色、字号、间距、圆角、总高。
//!
//! 与候选窗的 [`Palette`](super::Palette) 同源取色（都对着系统语义色），将来主题文件落地时
//! 两者是同一个文件的两节。所有数值单位是**点**。
//!
//! 配色与几何是照着安卓上主流输入法（讯飞那一类）的实机截图量的，不是随手定的：
//! 底色浅灰、内容键近白、功能键中灰、**回车是整块键盘上唯一的饱和色**。
//! 缝**横竖不一样宽**——行之间要透气，列之间挤一点键才够宽。

use crate::color::Color;
use crate::theme::FontSpec;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KeyboardTheme {
    /// 键盘底色（键帽之间的缝）。
    pub background: Color,

    /// 内容键（字母 / 数字 / 符号 / 空格）的键帽。
    pub key: Color,

    /// 内容键按住时。
    pub key_pressed: Color,

    /// 功能键（上档 / 退格 / 中英 / 逗号 / 切页）的键帽。
    pub key_function: Color,

    /// 功能键按住时。
    ///
    /// **单独一个色**，不复用 [`Self::key_pressed`]：功能键本来就是灰的，
    /// 按下去换成内容键那个浅灰反而**变亮**了，看着像没按。
    pub key_function_pressed: Color,

    /// 回车键的键帽。
    pub key_primary: Color,

    /// 回车键按住时。
    pub key_primary_pressed: Color,

    /// 键帽上的字与图标。
    pub label: Color,

    /// 键帽上角那个小字的颜色。
    ///
    /// 比 [`Self::label`] 淡——它是「这个键还能打什么」的提示，不是这个键本身。
    pub label_hint: Color,

    /// 大写的 Shift 锁定态用的强调色。
    pub accent: Color,

    /// 键帽上的字。
    pub font: FontSpec,

    /// 键帽角上那个小字的字号。
    pub hint_font: FontSpec,

    /// 笔画加深的 gamma——与候选窗用同一个值，两处的字重才一致。
    pub text_gamma: f32,

    /// 键与键之间的**横**缝（点）。
    pub gap_x: f32,

    /// 行与行之间的**竖**缝（点）。
    pub gap_y: f32,

    /// 键帽圆角（点）。
    pub radius: f32,

    /// 键盘总高（点）。行高按行数均分。
    pub height: f32,
}

impl KeyboardTheme {
    pub const fn light() -> Self {
        Self {
            background: Color::rgb(220, 224, 228),
            key: Color::rgb(252, 252, 252),
            key_pressed: Color::rgb(198, 203, 212),
            key_function: Color::rgb(188, 192, 204),
            key_function_pressed: Color::rgb(158, 163, 176),
            key_primary: Color::rgb(0, 188, 16),
            key_primary_pressed: Color::rgb(0, 148, 13),
            // 键帽上是黑字（截图量出来就是纯黑），只是略收一点不透明度让它不扎眼
            label: Color::gray(0, 235),
            label_hint: Color::gray(0, 185),
            accent: Color::rgb(0, 122, 255),
            font: FontSpec::new(17.0, 22.0),
            hint_font: FontSpec::new(9.0, 11.0),
            text_gamma: 0.85,
            gap_x: 7.0,
            gap_y: 11.0,
            radius: 5.0,
            height: 202.0,
        }
    }

    pub const fn dark() -> Self {
        Self {
            background: Color::rgb(30, 30, 32),
            key: Color::rgb(107, 107, 110),
            key_pressed: Color::rgb(150, 150, 155),
            key_function: Color::rgb(70, 70, 73),
            key_function_pressed: Color::rgb(108, 108, 112),
            key_primary: Color::rgb(0, 168, 20),
            key_primary_pressed: Color::rgb(0, 210, 28),
            label: Color::gray(255, 230),
            label_hint: Color::gray(255, 165),
            accent: Color::rgb(10, 132, 255),
            font: FontSpec::new(17.0, 22.0),
            hint_font: FontSpec::new(9.0, 11.0),
            text_gamma: 0.75,
            gap_x: 7.0,
            gap_y: 11.0,
            radius: 5.0,
            height: 202.0,
        }
    }

    /// 某个键帽的底色。`pressed` 时按**这个键本来的颜色**给按下色，
    /// 不是所有键共用一个——白键、灰键、绿键按下去都得看得出变化。
    pub const fn key_color(&self, style: crate::keyboard::KeyStyle, pressed: bool) -> Color {
        match (style, pressed) {
            (crate::keyboard::KeyStyle::Letter, false) => self.key,
            (crate::keyboard::KeyStyle::Letter, true) => self.key_pressed,
            (crate::keyboard::KeyStyle::Function, false) => self.key_function,
            (crate::keyboard::KeyStyle::Function, true) => self.key_function_pressed,
            (crate::keyboard::KeyStyle::Primary, false) => self.key_primary,
            (crate::keyboard::KeyStyle::Primary, true) => self.key_primary_pressed,
        }
    }
}
