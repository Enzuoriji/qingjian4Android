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

    /// 上屏**高亮那个候选的第几条译文**（点候选条底下那行小字，从 0 起）。
    ///
    /// 与 [`Self::CommitHighlighted`] 的区别只在「交出去的是哪串文本」：学习记账与拼音消耗
    /// 都由引擎按「选了那个候选」办（`Engine::commit_translation`），壳不自己拼。
    CommitTranslation(usize),

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

    /// 接受云联想给的整句补全（点候选条最下面那行右边那段）。
    ///
    /// 电脑上这一步是 Tab 键，安卓没有 Tab，所以给了个靶子。
    /// 上屏的文本由 `Engine::accept_prediction` 算（它还要记个人 n-gram），壳不自己拼。
    AcceptPrediction,

    /// 打开设置页（工具页那一格）。
    ///
    /// 会话**不自己开**——它碰不到安卓的窗口系统。这里只把「要开设置页」记一笔，
    /// 掩码里带上 `flags::SETTINGS`，由壳那边 `startActivity`。
    OpenSettings,

    /// 什么也不做。留给工具页上那些还没排工具的格子：点空处不该有反应
    /// （以前 `Tool(_)` 是通配到颜文字页，点哪儿都可能莫名跳一页）。
    Nothing,

    /// 把剪贴板第几条插到光标处（**整份里的下标**，不是本屏的；换算在壳里做）。
    PasteClipboard(usize),

    /// 删掉剪贴板里第几条（在记录上往左滑、松手）。
    DeleteClipboard(usize),

    /// 清空整份剪贴板历史。
    ClearClipboard,

    /// 表情页上点了一个：把那一格的东西上屏（**整条**，不是像打字那样一个字符一个字符）。
    Emoji(usize),

    /// 表情页上点了个分类标签：切到那一类。
    EmojiGroup(usize),

    /// 剪贴板那把锁：锁上之后点一条不弹回字母页（连着粘几条用）。
    ToggleClipboardLock,
}
