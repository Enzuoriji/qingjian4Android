//! 拼音没开着时，原样交给应用的按键。

/// 引擎没事可做、该把按键交还给应用的场合。
///
/// 这几件事**不能用上屏文本代替**：删字符得让应用自己删（输入法不知道光标前后有什么），
/// 回车在有些应用里是提交而不是插入换行。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// 删应用里的一个字符。
    Backspace,

    /// 回车：换行 / 提交 / 跳转，由应用自己解释。
    Enter,

    /// 光标左移一格。空格键上往左横滑出来的。
    MoveLeft,

    /// 光标右移一格。
    ///
    /// 移光标**不能用方向键**（`KEYCODE_DPAD_*` 在安卓上是**焦点导航**用的，光标到头时
    /// 会把焦点挪到界面按钮上），得走 `InputConnection.setSelection`——
    /// 这两个编号是给「往哪边挪」用的，具体挪法在壳那边。
    MoveRight,

    /// 把**光标前面整段**清掉。⌫ 上往上滑、松手时兑现。
    ///
    /// 清多少得问应用（输入法不知道光标前面有什么），壳用
    /// `InputConnection.deleteSurroundingText(光标前面有几个字, 0)` 一次删完。
    ClearToStart,
}

impl Command {
    /// 与 Kotlin 侧 `QingjianNative.COMMAND_*` 一一对应的编号。跨语言只传数字。
    pub const fn code(self) -> i32 {
        match self {
            Self::Backspace => 1,
            Self::Enter => 2,
            Self::MoveLeft => 3,
            Self::MoveRight => 4,
            Self::ClearToStart => 5,
        }
    }
}
