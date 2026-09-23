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

    /// 逗号键。键帽画**全角**「，」，交出去的仍是半角 `,`。
    Comma,

    /// 句号键。键帽画**全角**「。」，交出去的仍是半角 `.`。
    ///
    /// 与 [`KeyId::Comma`] 是一对，本可以都写成 [`KeyId::Literal`]——区别只在**键帽画全角**：
    /// 字母页底下这两个是最常用的标点，画全角一眼认得出来。符号页那些符号仍走 `Literal`，
    /// 画半角原字符（英文模式下不会画着全角却打出半角）。
    Period,

    /// 回车：上屏拼音原文。
    Enter,

    /// 打一个字符：数字、符号、标点都是它。
    ///
    /// 存的是**半角原字符**，全角与否交给引擎按设置转（`qingjian-core` 的标点表）——
    /// 键帽也照这个画，这样英文模式下不会画着全角却打出半角。
    Literal(char),

    /// 切到另一页。标签写的是**要去哪一页**，不是现在在哪页。
    Panel(Panel),

    /// 工具页上的一格（`TOOLS` 里第几个）。现在只有「剪贴板」，以后排设置。
    Tool(usize),

    /// 剪贴板页上的**视口**第几格（从 0 起，不是整份里的第几条——那份偏移在
    /// [`KeyboardState::clipboard_scroll`] 里）。文本从 [`KeyboardState`] 里取，
    /// 滚到头、这一格没内容时那格不画。
    ///
    /// [`KeyboardState`]: super::KeyboardState
    /// [`KeyboardState::clipboard_scroll`]: super::KeyboardState::clipboard_scroll
    Clipboard(usize),

    /// 清空整份剪贴板历史。
    ClipboardClear,

    /// 剪贴板的**锁**：锁上之后点一条不弹回字母页，可以连着粘几条。
    ClipboardLock,

    /// 表情页上的第几个格子（**屏幕上那一格**）。字符从 `KeyboardState` 里取。
    Emoji(usize),

    /// 表情页上面那条分类标签里、**这一屏**的第几个（点一下切到那一类）。
    ///
    /// 整条标签是能横着滑的（K13 ②，2026-09-23），所以「第几个」是**屏内**的下标——
    /// 会话按当前位移换算成整份分类里的第几个，渲染则把这个位移画进格子的位置上。
    EmojiGroup(usize),
}

/// 按键的样式。只影响配色，不影响行为。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyStyle {
    /// 内容键：字母、数字、符号、空格。白的。
    Letter,

    /// 功能键：Shift、退格、中 / 英、逗号、切页。灰的。
    Function,

    /// 回车。整块键盘上唯一的饱和色。
    Primary,
}

/// 键有多宽。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum KeyWidth {
    /// 占几个标准单位。
    Units(f32),

    /// 撑满这一行**剩下的**宽度。一行里最多一个。
    ///
    /// 键少的那一排靠它跟别的排一样宽：字母页最下一排只有 7 个键（6 条缝），
    /// 第 1、3 行有 10 个（9 条缝），全按固定单位排下来整排会窄一条、两头各缩进去半个键。
    /// 空格吃掉多出来的那一段，两头就对齐了。
    ///
    /// 这个做法是照 fcitx5-android 的：它的空格键 `percentWidth = 0`，
    /// 代码里写着 `0f means fill remaining space`。
    Fill,
}

/// 键盘上的一个键。
#[derive(Debug, Clone, Copy)]
pub struct Key {
    pub id: KeyId,

    /// 宽度。普通键 [`KeyWidth::Units`]，空格是 [`KeyWidth::Fill`]。
    pub width: KeyWidth,

    /// 键帽上方那个小字（角标）。没有角标就是 `None`。
    ///
    /// **它只是数据，不决定怎么触发。** 2026-09-21 之前是「在键上往下滑」打出来；
    /// 那个手势真机快打会误蹦符号、已撤掉，现在由平台层拿它当**长按那排选项**里的一项。
    ///
    /// 存的是**半角原字符**，与 [`KeyId::Literal`] 同一条路——全角与否交给引擎按设置转。
    pub hint: Option<char>,
}

impl Key {
    pub const fn new(id: KeyId, weight: f32) -> Self {
        Self {
            id,
            width: KeyWidth::Units(weight),
            hint: None,
        }
    }

    /// 一个撑满这一行剩余宽度的键（见 [`KeyWidth::Fill`]）。
    pub const fn fill(id: KeyId) -> Self {
        Self {
            id,
            width: KeyWidth::Fill,
            hint: None,
        }
    }

    /// 这个键占几个标准单位。`Fill` 那个算 **0**——它拿的是剩下的，
    /// 不参与「一个单位多宽」的计算，否则会跟自己的宽度循环论证。
    pub const fn units(&self) -> f32 {
        match self.width {
            KeyWidth::Units(weight) => weight,
            KeyWidth::Fill => 0.0,
        }
    }

    /// 一个标准宽的字母键，键帽上角标着 `hint`。
    pub const fn letter(c: char, hint: char) -> Self {
        Self {
            id: KeyId::Letter(c),
            width: KeyWidth::Units(1.0),
            hint: Some(hint),
        }
    }

    /// 这个键按哪种样子画。
    pub const fn style(&self) -> KeyStyle {
        match self.id {
            // 数字、符号、空格、工具与剪贴板的格子跟字母一样是「内容键」，白的
            KeyId::Letter(_)
            | KeyId::Literal(_)
            | KeyId::Space
            | KeyId::Tool(_)
            | KeyId::Clipboard(_)
            // 表情格子与分类标签都是「内容」：白的，点着才显眼
            | KeyId::Emoji(_)
            | KeyId::EmojiGroup(_) => KeyStyle::Letter,
            // 回车是唯一的强调键
            KeyId::Enter => KeyStyle::Primary,
            KeyId::Shift
            | KeyId::Backspace
            | KeyId::Mode
            | KeyId::Comma
            | KeyId::Period
            | KeyId::Panel(_)
            | KeyId::ClipboardClear
            | KeyId::ClipboardLock => KeyStyle::Function,
        }
    }
}
