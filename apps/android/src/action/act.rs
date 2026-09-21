//! 一次输入事件该做什么。

use qingjian_render::Panel;

/// 碰到一个键、或候选条上的一块之后该干的事。
///
/// 只说「做什么」，不说「做的时候引擎在什么状态」——那部分由壳执行时按当时的状态判断：
/// 退格是删拼音还是删应用里的字、空格是上屏候选还是打出空格，都要看拼音还开不开着。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Act {
    /// 喂一个字母给引擎。
    Push(char),

    /// 上屏候选：**本页**第几个（与渲染器报的页内下标一致，换算成跨页下标是壳的事）。
    CommitCandidate(usize),

    /// 上屏高亮那个候选；一个候选都没有时把空格本身交出去。
    CommitHighlighted,

    /// 把拼音原文上屏；拼音已经空了就把回车交给应用。
    CommitRaw,

    /// 删一个字母；拼音已经空了就把退格交给应用。
    Backspace,

    /// 清空拼音。
    Clear,

    /// 翻页，正数往后。
    Page(isize),

    /// 上档锁定 / 解锁。
    ToggleShift,

    /// 中 / 英切换。
    ToggleMode,

    /// 打一个标点或数字：给的是**半角原字符**，转不转全角由引擎按设置定。
    Punctuate(char),

    /// 切到键盘的另一页。
    SwitchPanel(Panel),

    /// 开 / 收工具页（候选条左边那个标）。在工具页与剪贴板页时收回字母页。
    ToggleTools,

    /// 把剪贴板第几条插到光标处（**整份里的下标**，不是本屏的；换算在壳里做）。
    PasteClipboard(usize),

    /// 删掉剪贴板里第几条（在记录上往左滑、松手）。
    DeleteClipboard(usize),

    /// 清空整份剪贴板历史。
    ClearClipboard,

    /// 剪贴板翻页，正数往后。
    ClipboardPage(isize),
}
