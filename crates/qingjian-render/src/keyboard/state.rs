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

/// 表情面板上面那条分类标签上，一格画什么。
///
/// 2026-09-23 之前标签行是一屏五个文字格、靠横滑看后面的分类；现在**全部分类平铺一行**
/// （照 fcitx5-android），所以一格里塞不下「Smileys & Emotion」这种长名字——改画图标。
/// （颜文字那页后来整个不分类了，那条标签行都不要了，见 `KeyboardLayout::kaomoji`。）
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupLabel {
    /// 画图标。
    Icon(GroupIcon),

    /// 画文字（分类名，两个字以内）。
    ///
    /// 字符串是**自己存一份**的：会话那边这个表跟着面板一起长期放着，
    /// 借分类名的话就成了自引用，存不下来。
    Text(String),
}

/// 分类标签能画的那几个图标。
///
/// 名字对的是随包表情表的分类（`assets/emoji/emoji-panel.tsv` 里那九个 + 「最近」）。
/// **「哪个分类用哪个图标」由会话层定**，渲染器只认这几个枚举值——它不该知道
/// 分类叫什么名字（那是随包数据的事，换一份表就变了）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupIcon {
    /// 「最近」。
    Recent,

    /// 笑脸与情绪。
    Smile,

    /// 人与身体。
    People,

    /// 动物与自然。
    Animals,

    /// 食物与饮料。
    Food,

    /// 旅行与地点。
    Travel,

    /// 活动。
    Activities,

    /// 物件。
    Objects,

    /// 符号。
    Symbols,

    /// 旗帜。
    Flags,
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

    /// 表情页要画的**那三页**：`[前一页, 当前页, 后一页]`，**当前页永远在中间**。
    ///
    /// 两头没有的那一侧给空切片。每页最多 [`crate::EMOJI_COLS`] × [`crate::EMOJI_ROWS`]
    /// 个字符，会话切好的。
    ///
    /// 横滑跟手时相邻那一页要跟着露出来，所以给的不止一页——**只给当前页的话，
    /// 滑到一半旁边就是空白**。
    pub emoji_pages: [&'a [String]; 3],

    /// 当前页的左边缘相对视口左边缘的偏移（**点**，正数 = 内容往左走 = 在看后一页）。
    ///
    /// 只有跟手的零头与收尾动画的中间值，**整页那部分由会话换掉**（`emoji_pages` 给的
    /// 就已经是这几页）——与 [`Self::clipboard_offset`] 一个道理，**不做除法**。
    pub emoji_shift: f32,

    /// 表情页上面那条分类标签，**全部分类**（不再只是看得见的那几个）。
    pub emoji_groups: &'a [GroupLabel],

    /// 这些标签里，当前这一页属于第几个（画成选中态）。
    pub emoji_group: usize,

    /// 分页细条：**当前这一类**翻到第几页了、一共几页——`(已经过的页数, 总页数)`，
    /// 第一个数是**连续的**（跟手时带小数），细条才跟得住手指。
    ///
    /// 类内只有一页时给 `None`：没什么可指示的，不画。
    pub emoji_pager: Option<(f32, usize)>,

    /// 「清空」那一下点过了、正等第二下确认。
    ///
    /// 剪贴板的清空要按两下（第一下手滑就清光所有历史太狠），第一下之后键帽改口说
    /// 「确认清空」——画什么由它定，什么时候过期由会话定（那边有钟）。
    pub clear_armed: bool,

    /// 剪贴板列表**让开不足一格的那点**（点，0 到一格高之间）。
    ///
    /// 整格的那部分由壳切好（[`Self::clipboard`] 给的就已经是这一屏该画的几条），
    /// 这里只剩零头——渲染器拿它把整排卡片往上让一让，**不做除法**：
    /// 「第几条起」是壳按键盘几何算的，两边各算一次会因浮点差出一格。
    pub clipboard_offset: f32,
}
