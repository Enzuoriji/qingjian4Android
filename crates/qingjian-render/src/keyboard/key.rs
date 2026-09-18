//! 键盘上的一个按键：它是什么、占多宽、长什么样。

/// 按键的身份。
///
/// 渲染器**只认身份，不知道按下去该干什么**——把 `KeyId` 翻成「喂给引擎」「上屏」「清空」
/// 属于平台层的输入翻译（安卓壳在 `apps/android/src/action/`），渲染器不该知道上屏是什么。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyId {
    /// 字母键。
    Letter(char),

    /// 上档：单击锁定、再击解锁。
    Shift,

    /// 退格。
    Backspace,

    /// 中 / 英切换。
    Mode,

    /// 空格：上屏高亮候选。
    Space,

    /// 逗号键（中文模式下出全角）。
    Comma,

    /// 回车：上屏拼音原文。
    Enter,
}

/// 按键的样式。只影响配色，不影响行为。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyStyle {
    /// 字母键。
    Letter,

    /// 功能键：Shift、退格、中 / 英、逗号。
    Function,

    /// 主键：空格、回车。
    Primary,
}

/// 键盘上的一个键。
#[derive(Debug, Clone, Copy)]
pub struct Key {
    pub id: KeyId,

    /// 宽度占几个标准单位。普通键 1.0，Shift 与回车 1.5，空格 5.0。
    pub weight: f32,
}

impl Key {
    pub const fn new(id: KeyId, weight: f32) -> Self {
        Self { id, weight }
    }

    /// 一个标准宽的字母键。
    pub const fn letter(c: char) -> Self {
        Self::new(KeyId::Letter(c), 1.0)
    }

    /// 这个键按哪种样子画。
    pub const fn style(&self) -> KeyStyle {
        match self.id {
            KeyId::Letter(_) => KeyStyle::Letter,
            KeyId::Space | KeyId::Enter => KeyStyle::Primary,
            KeyId::Shift | KeyId::Backspace | KeyId::Mode | KeyId::Comma => KeyStyle::Function,
        }
    }
}
