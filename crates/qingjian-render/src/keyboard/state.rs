//! 键盘的瞬时状态：按下了哪个键、Shift 在哪一档、中还是英。壳每次触摸后传一份过来。

use super::key::KeyId;

/// Shift 的三档。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShiftState {
    /// 没按。
    #[default]
    Off,

    /// 按了一次，下一个字母大写，之后自动回到 [`ShiftState::Off`]。
    Once,

    /// 锁定，一直大写。
    Locked,
}

impl ShiftState {
    /// 字母键现在该显示大写还是小写。
    pub const fn is_upper(self) -> bool {
        matches!(self, Self::Once | Self::Locked)
    }
}

/// 中 / 英模式。
///
/// 桌面上这个状态挂在 Caps Lock 位置的中 / 英键上；安卓是键盘自己的中 / 英键
/// （切到别的输入法是安卓系统的事，不归我们）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InputMode {
    #[default]
    Chinese,

    English,
}

/// 键盘此刻长什么样。
#[derive(Debug, Clone, Copy, Default)]
pub struct KeyboardState<'a> {
    pub shift: ShiftState,

    pub mode: InputMode,

    /// 正被按住的键，画成按下态。抬起或滑出后为 `None`。
    pub pressed: Option<KeyId>,

    /// 剪贴板历史，**整份**（最新在最前）。剪贴板页画的字就是从这儿来的。
    ///
    /// 渲染器只读它——**存与不存、留几条都不归它管**。
    pub clipboard: &'a [String],

    /// 剪贴板列表**让开不足一格的那点**（点，0 到一格高之间）。
    ///
    /// 整格的那部分由壳切好（[`Self::clipboard`] 给的就已经是这一屏该画的几条），
    /// 这里只剩零头——渲染器拿它把整排卡片往上让一让，**不做除法**：
    /// 「第几条起」是壳按键盘几何算的，两边各算一次会因浮点差出一格。
    pub clipboard_offset: f32,
}
