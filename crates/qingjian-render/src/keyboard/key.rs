//! 键盘上的一个按键：它是什么、占多宽、长什么样。

use super::panel::Panel;

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

    /// 打一个字符：数字、符号、标点都是它。
    ///
    /// 存的是**半角原字符**，全角与否交给引擎按设置转（`qingjian-core` 的标点表）——
    /// 键帽也照这个画，这样英文模式下不会画着全角却打出半角。
    Literal(char),

    /// 切到另一页。标签写的是**要去哪一页**，不是现在在哪页。
    Panel(Panel),
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
            // 数字与符号跟字母一样是「内容键」，白的
            KeyId::Letter(_) | KeyId::Literal(_) => KeyStyle::Letter,
            KeyId::Space | KeyId::Enter => KeyStyle::Primary,
            KeyId::Shift | KeyId::Backspace | KeyId::Mode | KeyId::Comma | KeyId::Panel(_) => {
                KeyStyle::Function
            }
        }
    }
}
