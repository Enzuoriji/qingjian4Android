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
    /// 移光标**只能用方向键**：输入法不知道光标前后有什么，也没有「把光标挪一格」的 API，
    /// 交给应用自己按它那边的规矩走（文本框、网页输入框、代码编辑器反应都不一样）。
    MoveRight,
}

impl Command {
    /// 与 Kotlin 侧 `QingjianNative.COMMAND_*` 一一对应的编号。跨语言只传数字。
    pub const fn code(self) -> i32 {
        match self {
            Self::Backspace => 1,
            Self::Enter => 2,
            Self::MoveLeft => 3,
            Self::MoveRight => 4,
        }
    }
}
