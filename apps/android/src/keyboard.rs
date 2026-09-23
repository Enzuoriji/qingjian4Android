//! 键盘这台前台：自己画键盘位图、自己算命中、自己记「哪根手指按着哪个键」。
//!
//! 单独成一层是为了**换得掉**。`Session` 只管引擎与候选条，键盘长什么样、触摸怎么算
//! 全在这里；将来若要改用安卓原生控件拼键盘（每个键一个 View），动的就是这一个文件——
//! `Session` 把这个字段置 `None`，位图那条路自然断掉。
//!
//! 分工：`Session::touch` 把**键盘那半边**的触摸转进来（坐标已减掉候选条高度），
//! 这里只回答「抬起来时兑现的是哪个键」；翻成动作在 `crate::action`，执行在 `Session`。

use qingjian_render::{
    GroupLabel, InputMode, Key, KeyHit, KeyId, KeyboardLayout, KeyboardState, KeyboardTheme, Popup,
    Rendered, RenderedKeyboard, Renderer, ShiftState,
};

use crate::surface;
use crate::touch::{MotionAction, TOUCH_SLOP, within_slop};

/// 手指从按下那点挪够这么远（点），就算「这是一记手势，不是点击」。
///
/// **现在只剩一处用它**：⌫ 往上滑（松手把光标前面整段清掉）。比的是**纵向**位移，
/// 所以这个阈值该跟**键高**比，不是跟键宽比——键高竖屏上有 42~55 点（跟着屏幕自适应，
/// 见 `KeyboardTheme::fitted`），半步 21~27 点，22 落在**键内**：
/// 手势认出来之前，这一下都还算「按着这个键」。往上一滑是**破坏性**的，阈值宁可高一点。
///
/// **取角标不再走滑动**（2026-09-21 撤掉，见 [`Chooser`]）。那个手势治不好：
/// 真机上快打会误蹦符号，而「手指出键 = 作废」与阈值线是**重合**的（都在半个键宽上），
/// 横向结构上救不了，把方向收成只认往下也只是压概率。改成长按弹一排选项之后，
/// 误触面直接归零——长按是有意的动作，快打根本按不到 300ms（壳的 `REPEAT_DELAY_MS`）。
pub(crate) const SWIPE: f32 = 22.0;

/// 空格上横滑的**死区**（点）：位移不到这儿不算移光标，仍然是「按空格」。
pub(crate) const CURSOR_DEAD_ZONE: f32 = 8.0;

/// 光标拖动最慢 / 最快时**每一拍**走几格。
///
/// 「一拍」是壳那边 50ms 的心跳，所以 0.2 格/拍约合 4 格/秒、2 格/拍是 40 格/秒。
/// 这个数和壳的 [`REPEAT_INTERVAL_MS`](crate::keyboard) 是一对，改一个要想着另一个。
const CURSOR_SLOWEST: f32 = 0.2;
const CURSOR_FASTEST: f32 = 2.0;

/// 位移涨到这么大（点）就到最快了。
const CURSOR_FAST_AT: f32 = 80.0;

/// 一次拖动最多连着走这么多格——手指一直按着不放也不至于把光标甩到天边。
const CURSOR_MAX_STEPS: isize = 400;

/// ⌫ 上**往上滑**这么多点，就是「要清掉光标前面整段」——**松手时**兑现。
///
/// 与 [`SWIPE`] 用同一个数：两处都是「手指纵向挪够了，这是手势不是点击」。
/// 往上一滑就清掉光标前面一整段是**破坏性**的，阈值宁可高一点。
///
/// **滑上去只是「预备」，松手落在气泡里才真清**（2026-09-23 用户要的）——
/// 见 `Keyboard::popup_rect`：气泡是个目标区，不是单纯的提示。
const CLEAR_SWIPE: f32 = SWIPE;

/// ⌫ 上往上滑之后，气泡上写这句话。
///
/// 键上那个退格图标说不出「松手会怎样」——滑上去之后这一下已经不是「删一个字」了，
/// 气泡得把话讲清楚，不然用户不知道松手会发生什么。
const CLEAR_HINT: &str = "松手清空";

/// 在剪贴板一条记录上**往左滑**多远算「要删这条」（点）——松手才删，跟 ⌫ 上滑一个手感。
///
/// 比 [`SWIPE`] 小：手指本来就靠格子左边按下去，往左一划就到头了（格子比键宽不了多少，
/// 一屏还分两格），阈值大了够不着。
const DELETE_SWIPE: f32 = 16.0;

/// 那条记录上往左滑过之后，气泡改口说什么。
const DELETE_HINT: &str = "松手删除";

/// 剪贴板记录区**上下滑**这么多点（纵向占优时）就算「在滚列表」。
///
/// 比 [`DELETE_SWIPE`] 还小：滚动是**跟手**的，手指一动就该动，等滑够十几点才有反应
/// 会很黏。两个手势按**方向**分（纵向占优滚、横向往左删），所以阈值小也不会互相误触。
const SCROLL_SLOP: f32 = 8.0;

/// 长按字母键弹的那排选项：**三格 —— 大写 / 符号 / 小写**，默认停在中间那个（符号）。
///
/// 2026-09-21 定的，取代原先「在键上往下滑 22 点取角标」。换掉的理由是那个手势**治不好**：
/// 真机上快打仍会误蹦符号（阈值降到 22 点、方向收成只认往下，都只是把概率压低），
/// 而长按是**有意**的动作——快打根本按不到 300ms（壳的 `REPEAT_DELAY_MS`），误触面直接归零。
/// 交互照搜狗那套：长按弹气泡、气泡里左右滑选、松手输入所选。
struct Chooser;

impl Chooser {
    /// 一格。
    const COUNT: usize = 3;

    /// 默认停在第几个——中间那个「符号」，也就是原先滑动要打的那个。
    const DEFAULT: usize = 1;

    /// 手指横着离开按下那点多远（点）就换一格。
    const STEP: f32 = 16.0;

    /// 那排画什么字。没有角标的键不开这一排（见 [`Keyboard::begin_choice`]），所以 `hint` 一定有。
    fn items(letter: char, hint: char) -> [char; Self::COUNT] {
        [
            letter.to_ascii_uppercase(),
            hint,
            letter.to_ascii_lowercase(),
        ]
    }

    /// 手指横着离开按下那点 `dx`（点）之后该选第几个。
    fn pick(dx: f32, density: f32) -> usize {
        let step = Self::STEP * density;
        if dx <= -step {
            0
        } else if dx >= step {
            2
        } else {
            Self::DEFAULT
        }
    }

    /// 选中那个兑现成哪个键。
    ///
    /// - **大写**走 `Literal`：要的是「中文模式下也直接打出一个大写字母」。
    ///   那条路会先把高亮候选上屏、再把字符原样交出去；字母不在标点表里，引擎原样透传。
    /// - **小写**走 `Letter`：就是「原来那个字母」，该进拼音缓冲区还进。
    /// - **符号**也走 `Literal`，与原来的角标同一个身份。
    fn fired(letter: char, hint: char, choice: usize) -> KeyId {
        match choice {
            0 => KeyId::Literal(letter.to_ascii_uppercase()),
            2 => KeyId::Letter(letter),
            _ => KeyId::Literal(hint),
        }
    }
}

/// 抬起来时兑现的东西。
///
/// 大多数时候是「按了某个键」，但空格键上横着滑是**移光标**——那不是某个键，
/// 翻成动作也就不是 [`crate::action::on_key`] 那条路，得分开报。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Fired {
    /// 按了某个键。
    Key(KeyId),

    /// 光标往右（正数）或往左（负数）移几格。
    MoveCursor(isize),

    /// 把光标**前面整段**清掉。⌫ 上往上滑、松手时兑现。
    ///
    /// 清多少由壳去问应用（输入法不知道光标前面有什么），这边只说「清」。
    ClearToStart,

    /// 删掉剪贴板里第几条（**整份里的下标**）。在一条记录上往左滑、松手时兑现。
    DeleteClipboard(usize),

    /// 剪贴板列表滚一下：手指这一拍上下挪了多少像素（往下为正）。
    ///
    /// 与空格移光标一样是**连续**的——拖动当中每一拍都要走，不是等松手才结算。
    ClipboardScroll(f32),

    /// 表情页的**格子区**横着挪一下：手指这一拍左右挪了多少像素（往右为正）。
    ///
    /// 与 [`Self::ClipboardScroll`] 一样是连续的。松手之后的收尾不在这儿报——
    /// 抬手时壳会调 `start_fling`，那边按「手速 + 拖了多远」定翻不翻页，再起一段吸附动画。
    EmojiPageScroll(f32),
}

/// 剪贴板那一摊：要画的那几条、让开的零头、清空确认、锁。
///
/// 打包成一个结构是因为 `surface` / `popup_surface` 都得收它——一个个传，
/// 调用处会变成一长串看不出谁是谁（`Keyboard::surface` 参数涨到 8 个就是这么来的）。
#[derive(Debug, Clone, Copy, Default)]
pub struct ClipboardView<'a> {
    /// 这一屏该画的那几条（会话按滚动量切好的）。
    pub entries: &'a [String],

    /// 让开不足一格的那点（点）——与 [`EmojiView::shift`] 同一个分工。
    pub offset: f32,

    /// 「清空」点过一下了、正等第二次确认。
    pub clear_armed: bool,

    /// 锁上了：点一条粘完**不回字母页**，可以连着粘几条。
    pub locked: bool,
}

/// 表情面板要画的那一堆：要摆的几页、上面那条标签、选中的是第几个，外加分页细条。
///
/// 打包成一个结构是因为 `surface` / `popup_surface` 都得收它——一个个传，
/// 调用处会变成一长串看不出谁是谁。
#[derive(Debug, Clone, Copy, Default)]
pub struct EmojiView<'a> {
    /// `[前一页, 当前页, 后一页]`——**当前页永远在中间**，两头没有的那一侧给空切片。
    ///
    /// 跟手时相邻那页要跟着露出来，所以不止一页：只给当前页的话，滑到一半旁边就是空白。
    pub pages: [&'a [String]; 3],

    /// 当前页左边缘相对视口的偏移（点，正数 = 内容往左走 = 在看后一页）。
    ///
    /// 只有跟手的零头与收尾动画的中间值——整页那部分由会话换掉（`pages` 给的就已经是这几页）。
    pub shift: f32,

    /// 分类标签，**全部**（不再只是看得见的那几个）。
    pub labels: &'a [GroupLabel],

    /// 当前这一页属于第几个分类（画成选中态）。
    pub group: usize,

    /// 分页细条：`(已经过的页数, 这一类的总页数)`。类内只有一页时给 `None`，不画。
    pub pager: Option<(f32, usize)>,
}

/// 尺寸与外观。壳在 `Session::configure` 时给一份。
#[derive(Debug, Clone, Copy)]
struct Metrics {
    /// 输入视图的宽度（点）。
    width: f32,

    /// **屏幕在当前方向上的高度**（点）——竖屏是长边、横屏是短边。键盘高度按它算。
    screen_height: f32,

    /// 屏幕密度（点 → 像素）。命中阈值按它换算。
    density: f32,

    /// 屏幕底部被系统手势条 / 导航栏占掉的高度（点）。键要往上让开这一段。
    bottom_inset: f32,

    /// 深色主题。
    dark: bool,

    /// 横屏。键矮一截，免得占掉半个屏幕。
    landscape: bool,
}

impl Default for Metrics {
    fn default() -> Self {
        Self {
            width: 0.0,
            screen_height: 0.0,
            density: 1.0,
            bottom_inset: 0.0,
            dark: false,
            landscape: false,
        }
    }
}

/// 一根按着的手指。
#[derive(Debug, Clone, Copy)]
struct Press {
    /// 安卓给的 pointer id。
    pointer: i32,

    /// 按下时命中的键。落在键之间的缝上时为 `None`。
    key: Option<KeyId>,

    /// 按下时的坐标。抬起时要靠它判断手指还在不在同一个键上。
    at: (f32, f32),

    /// 手指已经滑开了——**只影响画不画按下态**，不决定这一下算不算数。
    ///
    /// 上不上屏看的是**抬手那一刻手指在不在那个键的格子里**（见 `touch` 的 Up 分支）。
    /// 从前这里还兼着「整下作废」的活，于是反过来吃字：中途被划出键一拍（真机上快打
    /// 手指抖得快，很容易）就永远翻不了身，抬手回到键上也不算（2026-09-23 修的）。
    ///
    /// **不能直接把记录删掉**——删了抬起时就不知道刚才是从哪个键按下去的。
    sliding: bool,

    /// 这个键角上那个小字（符号）。没有角标就是 `None`。
    hint: Option<char>,

    /// **已经进「选一个」了**：长按字母键弹出的那排选项正开着。
    ///
    /// 长按（壳按够 `REPEAT_DELAY_MS` = 300ms 来问）从「连发」那条路上岔出来——字母键不连发，改开这一排。
    /// 开着的时候手指左右滑是**在这排里选**（见 [`Self::choice`]），不是滑键。
    choosing: bool,

    /// 选中的是那排里的第几个：**0 大写 / 1 符号 / 2 小写**，默认中间那个（符号）。
    ///
    /// 往左滑一格就到大写、往右滑一格就回小写，见 [`Chooser::pick`]。
    choice: usize,

    /// 这根手指此刻**横着离开了按下那点多远**（点，正数往右）。只有空格上移光标用得上。
    swipe: f32,

    /// 已经越过死区、进入移光标了——那这一下就不是「按空格」。
    cursor_started: bool,

    /// 移光标的小数累加器：速度是小数（慢的时候几拍才够一格），攒够一格才走。
    cursor_carry: f32,

    /// 已经往上滑够了：松手要把光标前面整段清掉。
    clearing: bool,

    /// 这根手指在剪贴板一条记录上往左滑过，这一下要删掉它（松手才删）。
    ///
    /// 与 [`Self::clearing`] 一样是**实时**判定：滑回原位就变回 `false`（反悔）。
    deleting: bool,

    /// 这根手指在剪贴板记录区**上下滑、滚列表**。
    ///
    /// 与 [`Self::deleting`] 互斥，按**方向**分：纵向位移占优的算滚（跟手走），
    /// 横向往左的算删。认了滚之后这一下就一直是滚了，不会再变回删。
    scrolling: bool,

    /// 上一拍手指在哪儿（纵向像素）。滚动是**增量**的（这一拍走多少），得记住上一下。
    last_y: f32,

    /// 上一次报上来的横坐标（像素）——表情页标签行横滑要的是**位移增量**，
    /// 与 [`Self::last_y`] 同理，只是方向换成横的。
    last_x: f32,

    /// 这根手指在标签行上**横着滑起来了**（K13 ②）。
    ///
    /// 越过死区才算，之后这一下就不再是「点了某个分类」。与 [`Self::scrolling`] 互斥：
    /// 表情页的标签行横着滑、格子区竖着滑，同一页两个方向。
    /// 置上之后 [`Keyboard::refresh_pressed`] 也不再认这根手指——滑起来是「浏览」，
    /// 不该画按下的样儿（含键帽上那个放大气泡）。
    scrolling_x: bool,

    /// 这根手指**起过手势**（上滑清空 / 左滑删除的阈值碰过），即使后来滑回来了也一直记着。
    ///
    /// 用处只有一个：松手时**别再当成「点了一下」**——反悔之后抬手该什么也不做。
    gestured: bool,

    /// 这一下已经连发过了。
    ///
    /// 连发过就不再按「点击」兑现——键是**抬起时**才触发一次的，按住删一串之后松手，
    /// 那一下会再删一个，等于每次都多退一格。
    repeated: bool,
}

/// 自绘的键盘前台。
pub struct Keyboard {
    /// 键盘布局，建一次就够。
    layout: KeyboardLayout,

    metrics: Metrics,

    /// 画好待用的键盘。命中矩形就在它里面，触摸时直接用，不必重画。
    rendered: Option<RenderedKeyboard>,

    /// 脏了没有——`surface` 被调用时才真重画。
    dirty: bool,

    /// 此刻按着的手指们，**按根记**。
    ///
    /// 快打时两根拇指的接触时间会重叠，只留一个「当前按下的键」的话，
    /// 后按下的那根会把前一根挤掉，两根的字母一起丢——真机上报的「点快了掉字母」就是它。
    presses: Vec<Press>,

    /// 正被按住的键，画成按下态。
    pressed: Option<KeyId>,

    /// 那一根手指**已经往上滑了**：松手要把光标前面整段清掉。气泡照它改口。
    pressed_clearing: bool,

    /// 正按着的那根手指在剪贴板一条记录上往左滑过（气泡要说「松手删除」）。
    pressed_deleting: bool,

    /// 那一根手指**长按开着那排选项**：气泡要画那一排，还得知道选中第几个。
    ///
    /// `None` = 没开着。有值时就是选中的下标（0 大写 / 1 符号 / 2 小写）。
    pressed_choice: Option<usize>,

    /// 画好的键预览气泡，以及它是**给哪个键、多大尺寸、什么形态**画的。
    ///
    /// 按住键那一下要弹；同一个键按着不动就不必重画（画一次 ~0.8ms，每拍重画白费）。
    /// 最后那个数是形态：0 普通键、1 「松手清空」、2 起是那排选项的第几个——
    /// 手指在选项里左右滑时它一直在变，靠它决定要不要重画。
    popup: Option<Rendered>,
    popup_for: Option<(KeyId, u32, u32, u32)>,
}

/// 空格上横滑时，位移 `dx`（点）下**一拍**该走几格（正数往右）。
///
/// **越远越快**：位移从死区涨到 [`CURSOR_FAST_AT`] 的过程中，速度在
/// [`CURSOR_SLOWEST`] 与 [`CURSOR_FASTEST`] 之间线性插值——手指挪得越远，
/// 光标走得一格比一格快，这是要的手感。
fn cursor_rate(dx: f32, dead_zone: f32) -> f32 {
    let distance = dx.abs();
    if distance < dead_zone {
        return 0.0;
    }
    let t = ((distance - dead_zone) / (CURSOR_FAST_AT - dead_zone)).clamp(0.0, 1.0);
    let rate = CURSOR_SLOWEST + (CURSOR_FASTEST - CURSOR_SLOWEST) * t;
    if dx < 0.0 { -rate } else { rate }
}

impl Keyboard {
    /// 建一台键盘前台，用缺省的字母布局。尺寸要等壳 [`Self::set_metrics`] 报过来才画得出。
    pub fn new() -> Self {
        Self {
            layout: KeyboardLayout::letters(),
            metrics: Metrics::default(),
            rendered: None,
            dirty: true,
            presses: Vec::new(),
            pressed: None,
            pressed_clearing: false,
            pressed_deleting: false,
            pressed_choice: None,
            popup: None,
            popup_for: None,
        }
    }

    /// 壳报告尺寸与明暗。
    ///
    /// 脏标记由这里自己管：几项都没变就不重画。`Session` 那边不必再判一次——
    /// 各判各的，省得谁忘了同步谁。
    pub fn set_metrics(
        &mut self,
        width: f32,
        screen_height: f32,
        density: f32,
        bottom_inset: f32,
        dark: bool,
        landscape: bool,
    ) {
        if (self.metrics.width - width).abs() > 0.5
            || (self.metrics.screen_height - screen_height).abs() > 0.5
            || (self.metrics.density - density).abs() > 0.01
            || (self.metrics.bottom_inset - bottom_inset).abs() > 0.5
            || self.metrics.dark != dark
            || self.metrics.landscape != landscape
        {
            self.metrics = Metrics {
                width,
                screen_height,
                density,
                bottom_inset,
                dark,
                landscape,
            };
            self.dirty = true;
        }
    }

    /// 键盘占多高（点），不含底部让开的那一段。**按屏幕高矮算**，见 `KeyboardTheme::fitted`。
    pub fn height(&self) -> f32 {
        self.theme().height
    }

    /// 剪贴板列表一格多高（点）：行高 + 行间那条缝。
    ///
    /// 与渲染器同一套算法（[`KeyboardLayout::row_height`]）——「滚到第几条起、还能滚多远」
    /// 都按它算，而键盘几何只有这儿知道。**渲染器那边不必再算一遍**：会话把这一屏该画的
    /// 那几条切好了喂过去，渲染器只负责让开不足一格的那点。
    pub fn clipboard_pitch(&self) -> f32 {
        let gap = self.theme().gap_y;
        KeyboardLayout::clipboard().row_height(self.height(), gap) + gap
    }

    /// 表情页**标签行**有多高（点）——它下面才是格子区。
    ///
    /// 与画的时候**同一套算法**（[`KeyboardLayout::row_height`]）：标签行是第 0 行，
    /// 各行等高。横滑的判定靠它划出「手指在不在标签行上」——**按行判而不是按格判**，
    /// 落在格子之间的缝里也得能滑（剪贴板那边 2026-09-21 吃过这个亏）。
    pub fn label_height(&self) -> f32 {
        KeyboardLayout::emoji().row_height(self.height(), self.theme().gap_y)
    }

    /// 标脏，下次 `surface` 重画。Shift 与中 / 英切换改的是键帽长相，由 `Session` 叫它。
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// 换一套布局（切页）。换了就得重画，连命中矩形一起换——那两个是一起出来的。
    pub fn set_layout(&mut self, layout: KeyboardLayout) {
        self.layout = layout;
        // 页换了，旧页上按着的手指对新页没有意义
        self.presses.clear();
        self.pressed = None;
        self.forget_popup();
        self.dirty = true;
    }

    /// 把画好的气泡丢掉，下次重画。
    fn forget_popup(&mut self) {
        self.popup = None;
        self.popup_for = None;
    }

    /// 键盘脏了没有——`Session` 据此决定要不要让壳重取位图。
    pub fn dirty(&self) -> bool {
        self.dirty
    }

    /// 键盘的位图（8 字节头 + 预乘 RGBA）。没配过宽度或渲染器不可用时给空。
    ///
    /// Shift 与中 / 英是引擎那头的状态（`Session` 拿着），画的时候借过来用一下；
    /// 剪贴板那一份也是（记录在会话里，键盘只负责画出来）；键盘自己只记「哪个键看着是按下的」。
    pub fn surface(
        &mut self,
        renderer: Option<&mut Renderer>,
        shift: ShiftState,
        mode: InputMode,
        clipboard: ClipboardView<'_>,
        emoji: EmojiView<'_>,
    ) -> Vec<u8> {
        if self.metrics.width <= 0.0 {
            return Vec::new();
        }
        if self.dirty || self.rendered.is_none() {
            let theme = self.theme();
            let state = KeyboardState {
                shift,
                mode,
                pressed: self.pressed,
                clipboard: clipboard.entries,
                clipboard_offset: clipboard.offset,
                clear_armed: clipboard.clear_armed,
                clipboard_locked: clipboard.locked,
                emoji_pages: emoji.pages,
                emoji_shift: emoji.shift,
                emoji_groups: emoji.labels,
                emoji_group: emoji.group,
                emoji_pager: emoji.pager,
            };
            let rendered = renderer.and_then(|renderer| {
                renderer
                    .render_keyboard(
                        &self.layout,
                        &state,
                        self.metrics.width,
                        self.metrics.bottom_inset,
                        &theme,
                        self.metrics.density,
                    )
                    .ok()
            });
            let Some(rendered) = rendered else {
                return Vec::new();
            };
            self.rendered = Some(rendered);
            self.dirty = false;
        }

        self.rendered.as_ref().map_or_else(Vec::new, |keyboard| {
            surface::encode(&keyboard.rendered.pixmap)
        })
    }

    /// 键盘那半边的触摸。`x` / `y` 是**键盘局部**的像素（壳已减掉候选条高度）。
    ///
    /// 只管**起手就落在键盘上**的手指：不在 `presses` 里的 pointer 一律不理，
    /// 所以壳可以把每个事件都送进来，不必自己记「这根手指是哪个区的」。
    ///
    /// 返回抬起来时兑现的东西（按了某个键，或者空格上横滑移光标）；没有就是 `None`。
    pub fn touch(&mut self, action: MotionAction, pointer: i32, x: f32, y: f32) -> Option<Fired> {
        match action {
            MotionAction::Down | MotionAction::PointerDown => {
                // 键盘在候选条下面；y 为负说明按到候选条那半边去了，不归这里管
                if y < 0.0 {
                    return None;
                }
                self.presses.retain(|press| press.pointer != pointer);
                self.presses.push(Press {
                    pointer,
                    key: self.hit(x, y),
                    at: (x, y),
                    sliding: false,
                    hint: self.hint_at(x, y),
                    choosing: false,
                    choice: Chooser::DEFAULT,
                    swipe: 0.0,
                    cursor_started: false,
                    cursor_carry: 0.0,
                    clearing: false,
                    deleting: false,
                    scrolling: false,
                    last_y: y,
                    last_x: x,
                    scrolling_x: false,
                    gestured: false,
                    repeated: false,
                });
                self.refresh_pressed();
                None
            }
            MotionAction::Move => {
                let hit = self.hit(x, y);
                let dead_zone = CURSOR_DEAD_ZONE * self.metrics.density;
                let clear_swipe = CLEAR_SWIPE * self.metrics.density;
                // 标签行的下边界（像素）。**先算出来**：下面要可变借 `presses`，
                // 那期间不能再借 `self` 问这个数
                let label_bottom = self.label_height() * self.metrics.density;
                if let Some(press) = self
                    .presses
                    .iter_mut()
                    .find(|press| press.pointer == pointer)
                {
                    let dy = y - press.at.1;
                    // **开着那排选项时，左右滑是在排里选**，不是滑键也不是点击。
                    // 手指滑出键外也不作废（这一下早就不在按键了），所以直接返回，
                    // 免得下面那条「出了键 = 作废」把气泡弄没。
                    if press.choosing {
                        let picked = Chooser::pick(x - press.at.0, self.metrics.density);
                        if picked != press.choice {
                            press.choice = picked;
                        }
                        self.refresh_pressed();
                        return None;
                    }
                    // ⌫ 上**往上滑** = 要清掉光标前面整段（**松手才清**，滑上去只是「预备」）。
                    // 挑往上而不是往左：往左是「退格」本来的方向，容易跟普通退格混；
                    // 往上是个独立的动作，不会误触。
                    //
                    // **滑回原位就取消**：这个判定每一下都重算，不是「滑过一次就定死」——
                    // 滑错了要能反悔（气泡也跟着改回来）。
                    if press.key == Some(KeyId::Backspace) {
                        press.clearing = dy <= -clear_swipe;
                    }
                    // 表情页的**标签行只点不滑**（2026-09-23 照 fcitx5 改）：横着划过去
                    // 什么也不做，**也不把这一下判成「滑出去了」**——标签格子窄（十个分类
                    // 平分一行），手指偏一点就跨到邻格，判成滑动的话点标签会经常点不中。
                    // 抬手时按**按下时命中的那一格**结算（点标签由抬起兑现）。
                    if self.layout.is_emoji() && press.at.1 < label_bottom {
                        return None;
                    }
                    // 表情页的**格子区**横着滑 = 翻页（同一天改的：原来滑的是上面那条标签，
                    // 现在滑下面这一大片——与 fcitx5 的 ViewPager2 一个位置）。
                    // 跟手：拖多少走多少；翻不翻、翻到哪，松手时由壳按「手速 + 拖了多远」判。
                    //
                    // 判的是**哪一页**，不是「按住了哪个格子」——手指落在格子之间的缝里
                    // 一样得能滑（剪贴板那边 2026-09-21 吃过这个亏）。
                    if self.layout.is_emoji() {
                        let dx = x - press.at.0;
                        let dy = y - press.at.1;
                        let slop = SCROLL_SLOP * self.metrics.density;
                        if press.scrolling_x || (dx.abs() >= slop && dx.abs() > dy.abs()) {
                            press.scrolling_x = true;
                            press.gestured = true;
                            let step = x - press.last_x;
                            press.last_x = x;
                            // 滑起来之后这一下就不再是「按住了某一格」，把按下态撤掉——
                            // 不然键帽上那个放大气泡会跟着手指跑一整路（用户 2026-09-23 指出的）。
                            // **必须赶在下面 return 之前调**：早先这一步写在后面，横滑那一路
                            // 提前 `return` 走掉了，压根没走到，气泡照旧。
                            self.refresh_pressed();
                            if step != 0.0 {
                                return Some(Fired::EmojiPageScroll(step));
                            }
                            return None;
                        }
                        // 竖着滑：表情页**没有上下滚了**（一页一格，多的横着翻），
                        // 但这一下也不再是「点了某一格」——不然按住一格上下划会误上屏。
                        if dy.abs() >= slop {
                            press.sliding = true;
                            press.gestured = true;
                        }
                        return None;
                    }
                    // 剪贴板记录区上两个手势，按**方向**分（同一块地方，先认出来的算数）：
                    // - **上下滑 = 滚列表**（跟手，每拍都要走）
                    // - **往左滑 = 要删这条**（松手才兑现、拖回原位就取消）
                    // 往左是「不要了」的方向，跟候选条上「往左看后面的候选」不冲突——那儿是另一块地方。
                    if self.layout.is_clipboard() {
                        let dx = x - press.at.0;
                        let dy = y - press.at.1;
                        let slop = SCROLL_SLOP * self.metrics.density;
                        if press.scrolling || (dy.abs() >= slop && dy.abs() > dx.abs()) {
                            press.scrolling = true;
                            press.gestured = true;
                            let step = y - press.last_y;
                            press.last_y = y;
                            if step != 0.0 {
                                return Some(Fired::ClipboardScroll(step));
                            }
                        } else {
                            // **只有剪贴板那几格**能「往左滑删一条」——表情页的格子没有
                            // 「删」这回事（emoji 是随包的数据，不是用户的东西）。
                            // 这一条是 2026-09-21 补的：把滚动判定扩到表情格时忘了把
                            // 删除留给剪贴板，于是表情格左滑会弹「松手删除」的气泡，
                            // 而抬手兑现的 `DeleteClipboard` 删的其实是**剪贴板**里第 N 条。
                            press.deleting = dx <= -DELETE_SWIPE * self.metrics.density;
                        }
                    }
                    // 手势**开过**就一直记着（哪怕又滑回来了）：这样松手不会当成「点了一下」——
                    // 反悔之后抬手该什么也不做，不该顺手把这条粘出去
                    if press.clearing || press.deleting || press.scrolling {
                        press.gestured = true;
                    }
                    // 空格上横着滑 = 移光标。**拖动当中就走**，不是等松手才算——
                    // 松手才走的话手指得先盲拖一段、再抬起来看结果，没法一边看一边调。
                    if press.key == Some(KeyId::Space) {
                        press.swipe = x - press.at.0;
                        if !press.cursor_started && press.swipe.abs() >= dead_zone {
                            // 刚越过死区**先走一格**：不然要等下一拍（最多 50ms）才有反应
                            press.cursor_started = true;
                            press.cursor_carry = 0.0;
                            let first = if press.swipe > 0.0 { 1 } else { -1 };
                            self.refresh_pressed();
                            return Some(Fired::MoveCursor(first));
                        }
                    }
                    // 键很大（三十多点宽），手指抖一抖不该掉字，所以「还落在这个键上」就一直算按着；
                    // 滑到别的键或键之间的缝上才取消。候选条那边不是这个判法，得挪出触摸阈值。
                    //
                    // **已经认成手势的那几根一律不取消**，哪怕手指滑出了键：
                    // - 选一个（`choosing`）：上面那条已经 `return` 了，这里只是再保一道
                    // - 移光标（`cursor_started`）：空格键宽，划着划着就出去了，那不是「这一下不要了」
                    // - ⌫ 上滑清空 / 剪贴板左滑删除（`gestured`）：**这条是补的**——往上一滑手指就出了 ⌫，
                    //   原先这里没排掉它，于是 `sliding` 一置上，`refresh_pressed` 就不认这根手指了，
                    //   气泡当场消失——用户正是靠气泡上那句「松手清空」才知道自己在干什么
                    // 「滑出去了」**每一拍重算**，不是一次置上就再也不回头：手指滚出去又
                    // 滑回键上，这一下还算数（同一个分支里 ⌫ 那个清空手势就是这么写的）。
                    // 一次置上不复位的写法在真机上掉字母——快敲时拇指滚出键帽一点点，
                    // 整下敲击就作废了，滑回来也补不回来（用户 2026-09-23 报的「漏字母」）。
                    if !press.choosing && !press.cursor_started && !press.gestured {
                        press.sliding = press.key.is_none() || hit != press.key;
                    }
                }
                self.refresh_pressed();
                None
            }
            MotionAction::Up | MotionAction::PointerUp => {
                let index = self
                    .presses
                    .iter()
                    .position(|press| press.pointer == pointer);
                // 气泡的矩形要**趁现在**取（下面一动 `presses`，「按着哪个键」就查不到了）
                let popup_rect = self.popup_rect();
                let ended = index.map(|index| self.presses.remove(index))?;
                self.refresh_pressed();
                // 长按开的那排：松手兑现选中的那个（大写 / 符号 / 小写）
                if ended.choosing {
                    let (Some(KeyId::Letter(letter)), Some(hint)) = (ended.key, ended.hint) else {
                        return None;
                    };
                    return Some(Fired::Key(Chooser::fired(letter, hint, ended.choice)));
                }
                // 按住连发过的，抬手不再补一下——不然后面总是多删一个字
                if ended.repeated {
                    return None;
                }
                // 空格上移过光标了：这一下已经在拖动当中兑现完了，抬手不再补一个空格
                if ended.cursor_started {
                    return None;
                }
                // ⌫ 上往上滑过：松手把光标前面整段清掉。**不是**按一下退格——
                // 那一下的语义是「清空前面」，跟删一个字不是一回事
                if ended.clearing {
                    // **松手要落在气泡里**才算数（2026-09-23 用户要的）：气泡不再只是提示，
                    // 还是个**目标**——滑上去、停在气泡上，松手才清；滑过头溜出气泡外面松手，
                    // 这一下什么也不做（跟拖回原位一个意思：反悔了）。
                    let inside = popup_rect.is_some_and(|(bx, by, bw, bh)| {
                        x >= bx && x < bx + bw && y >= by && y < by + bh
                    });
                    return inside.then_some(Fired::ClearToStart);
                }
                // 剪贴板那条往左滑过：松手删掉它
                if ended.deleting {
                    let Some(KeyId::Clipboard(index)) = ended.key else {
                        return None;
                    };
                    return Some(Fired::DeleteClipboard(index));
                }
                // 手势开过又滑回来了（反悔）：这一下什么也不做——别当成「点了一下」
                // （在剪贴板那格上，点一下是**粘出去**，反悔的人不会想粘）
                if ended.gestured {
                    return None;
                }
                // 抬起时只要还在那个键上、或者只挪了触摸阈值那么点距离，都算这一下按着了
                match ended.key {
                    // 抬手时手指还在**那个键的格子里**，这一下就算数（命中区已经铺到整格，
                    // 见 `render_keyboard` 里那段）。
                    //
                    // **不看 `ended.sliding`**：那是「中途有没有滑出去过」的粘性标记，
                    // 而真机上快打手指抖得快、系统的 MOVE 又不是每一像素都给——抬手那一下
                    // 明明已经回到键上了，标记还挂着，整下就白打了。用户 2026-09-23 报的
                    // 「有些字母依旧会漏掉」正是这种：抬手坐标跟最后一拍 MOVE 的坐标不一样
                    // 才露出来，测试里不特意构造就撞不上。
                    //
                    // 抬手落在别处时下面两条自然都不成立，所以「滑走了」照样不出。
                    Some(key)
                        if self.hit(x, y) == Some(key)
                            || within_slop(ended.at, self.touch_slop(), x, y) =>
                    {
                        Some(Fired::Key(key))
                    }
                    _ => None,
                }
            }
            MotionAction::Cancel => {
                self.presses.clear();
                self.refresh_pressed();
                None
            }
            // 不认识的事件：什么都不做（见 [`MotionAction::Ignore`]）
            MotionAction::Ignore => None,
        }
    }

    /// 某个键的命中矩形（**键盘局部**像素）。还没画过、或键盘上没这个键时为 `None`。
    ///
    /// 测试拿它取点位；键预览气泡拿它算摆哪儿。
    pub(crate) fn key_rect(&self, id: KeyId) -> Option<KeyHit> {
        self.rendered
            .as_ref()?
            .keys
            .iter()
            .find(|key| key.id == id)
            .copied()
    }

    /// 按住键时那张预览气泡的位图（8 字节头 + 预乘 RGBA）。没按住、或那个键没什么可预览的，就是空的。
    ///
    /// 壳收到空字节串要把浮动小窗收起来——跟候选条「空表示不该在」一个规矩。
    pub fn popup_surface(
        &mut self,
        renderer: Option<&mut Renderer>,
        shift: ShiftState,
        mode: InputMode,
        clipboard: ClipboardView<'_>,
        emoji: EmojiView<'_>,
    ) -> Vec<u8> {
        let Some((key, rect)) = self.pressed_key() else {
            self.forget_popup();
            return Vec::new();
        };
        // 空格没有字可显示，弹一个空框子只是晃眼
        if key.id == KeyId::Space {
            self.forget_popup();
            return Vec::new();
        }
        // 剪贴板那几格**不弹气泡**（2026-09-21 用户要的）：气泡正好压在下面那几条上，
        // 按住一条想看别的就碍事了；那儿也没什么要预览的——一条的内容本来就写在卡片上。
        //
        // **往左滑着要删的时候除外**（2026-09-23 补的那条例外）：上面那条说的是「平时」，
        // 结果把删除态也一起挡了（这段早退在 `pressed_deleting` 那个分支**之前**返回）——
        // 手指划过去只看得见卡片变个色，说不出松手会干什么。
        // 「松手删除」那句话的代码一直在下面，只是走不到这儿。
        if matches!(key.id, KeyId::Clipboard(_)) && !self.pressed_deleting {
            self.forget_popup();
            return Vec::new();
        }
        // 气泡画什么，按这一下**实际会干什么**来：
        // - 长按开着那排选项（只有带角标的字母键会开）→ 画那一排，选中的垫底
        // - ⌫ 上滑过 → 那句「松手清空」（退格图标说不出「松手会怎样」）
        // - 其余 → 这个键自己的样子
        let chooser = match (self.pressed_choice, key.id, key.hint) {
            (Some(index), KeyId::Letter(letter), Some(hint)) => {
                Some((Chooser::items(letter, hint), index))
            }
            _ => None,
        };
        let content = match &chooser {
            Some((items, index)) => Popup::Choices {
                items: *items,
                selected: *index,
            },
            None if self.pressed_clearing => Popup::Text(CLEAR_HINT),
            None if self.pressed_deleting => Popup::Text(DELETE_HINT),
            None => Popup::Key(&key),
        };
        // 形态：0 普通 / 1 松手清空 / 2 起是那排选项的第几个
        // （「松手删除」与「松手清空」同一形态：都是换一句话说，宽高按字量）
        let shape = match &chooser {
            Some((_, index)) => 2 + *index as u32,
            None if self.pressed_clearing || self.pressed_deleting => 1,
            None => 0,
        };

        let mark = (
            key.id,
            rect.width.round() as u32,
            rect.height.round() as u32,
            shape,
        );
        if self.popup_for != Some(mark) || self.popup.is_none() {
            let theme = self.theme();
            let state = KeyboardState {
                shift,
                mode,
                pressed: self.pressed,
                clipboard: clipboard.entries,
                clipboard_offset: clipboard.offset,
                clear_armed: clipboard.clear_armed,
                clipboard_locked: clipboard.locked,
                emoji_pages: emoji.pages,
                emoji_shift: emoji.shift,
                emoji_groups: emoji.labels,
                emoji_group: emoji.group,
                emoji_pager: emoji.pager,
            };
            let density = self.metrics.density;
            let rendered = renderer.and_then(|renderer| {
                renderer
                    .render_key_popup(
                        content,
                        &state,
                        rect.width / density,
                        rect.height / density,
                        &theme,
                        density,
                    )
                    .ok()
            });
            let Some(rendered) = rendered else {
                return Vec::new();
            };
            self.popup = Some(rendered);
            self.popup_for = Some(mark);
        }

        self.popup
            .as_ref()
            .map_or_else(Vec::new, |popup| surface::encode(&popup.pixmap))
    }

    /// 气泡位图**左上角**在键盘局部（像素）的坐标。没在预览时是 `None`。
    ///
    /// 摆哪儿在这边算：壳只把浮动小窗挪到「视图在屏幕上的位置 + 这个偏移」，不掺和布局
    /// ——与「命中测试在 Rust 里做」同一个规矩。
    /// 气泡**内容**的底边贴着键的上边、水平中心对齐键的中心；位图四周还留着阴影，减掉才是左上角。
    pub fn popup_origin(&self) -> Option<(f32, f32)> {
        let (_, rect) = self.pressed_key()?;
        let popup = self.popup.as_ref()?;
        // **位图**的左上角，不是**内容**的：位图四周还留着一圈阴影，得减掉。
        // 横向内容在位图里居中，所以按位图宽算；纵向内容底边对齐键顶，按内容在位图里的偏移算
        let bitmap = popup.pixmap.width() as f32;
        let x = rect.x + rect.width / 2.0 - bitmap / 2.0;
        let y = rect.y - popup.content_y as f32 - popup.content_height as f32;

        // 最左 / 最右那几个键（`符`、`回车`、`⌫`），气泡比键宽，居中就探出屏幕了——
        // 往里夹一下。夹的是整张位图，内容在里面居中，所以只是整体挪进来，不会变形
        let room = (self.metrics.width * self.metrics.density - bitmap).max(0.0);
        Some((x.clamp(0.0, room), y))
    }

    /// 那张气泡**该在**的矩形（**键盘局部**坐标，与触摸同一套）：`(左, 上, 宽, 高)`。
    ///
    /// 底边贴键顶、横向对键心——就是气泡的本来位置。**不是画出来的那个位置**：
    /// 最左 / 最右那几个键上气泡会被夹回屏幕里（见 [`Self::popup_origin`] 的防出屏），
    /// 而手指照旧停在键心正上方——拿夹过的位置判，`⌫`（在最右边）就永远落不进气泡，
    /// 「松手清空」会彻底失灵（2026-09-23 在测试里量出来的：手指在 x=924，
    /// 夹过的气泡只有 [666, 908]）。
    ///
    /// 用的也是**可见的那块内容**、不是整张位图：位图四周留着一圈透明阴影，
    /// 算进去的话手指落在阴影上也算数。
    fn popup_rect(&self) -> Option<(f32, f32, f32, f32)> {
        let (_, rect) = self.pressed_key()?;
        let popup = self.popup.as_ref()?;
        let (width, height) = (popup.content_width as f32, popup.content_height as f32);
        Some((
            rect.x + rect.width / 2.0 - width / 2.0,
            rect.y - height,
            width,
            height,
        ))
    }

    /// 此刻正按住的那个键：**布局数据、命中矩形、以及它在这张位图里多大**。
    ///
    /// 三样一起给，是因为键预览气泡三样都要——分三次查不如一次给全。
    /// 没按住键、或者键盘还没画过时为 `None`。
    pub fn pressed_key(&self) -> Option<(Key, KeyHit)> {
        let id = self.pressed?;
        let rect = self.key_rect(id)?;
        let key = self
            .layout
            .rows()
            .iter()
            .flat_map(|row| row.keys.iter())
            .find(|key| key.id == id)?;
        Some((*key, rect))
    }

    /// 这根手指此刻**按住**的键。没按在键上、已经滑开、或者已经变成手势了，都是 `None`。
    ///
    /// 给长按连发用：连发要问的是「这根手指还在老老实实按着这个键吗」，
    /// 而不是 [`Self::pressed`] 那个「最后按下的是哪个键」——后者是给键帽上色用的，
    /// 两根手指交替时它会被后按下的那根挤掉。
    ///
    /// **手势一开就不算「按住」**：空格移光标、退格上滑清空，这两样都是从
    /// 「按住这个键」岔出去的路。不排掉的话，滑得慢一点（超过连发门槛 300ms）
    /// 就会一边做手势一边被连发删——实测踩到过。
    ///
    /// （原先这条还列着「滑动取角标」，2026-09-21 那个手势撤掉、改成长按弹选项之后就不存在了。）
    ///
    /// 看的是 `gestured`（起过手势就一直算），不是 `clearing` 那个实时值——
    /// 否则手指滑回来时又变回「按住」，连发会在半路接上。
    pub fn held(&self, pointer: i32) -> Option<KeyId> {
        self.presses
            .iter()
            .find(|press| {
                press.pointer == pointer
                    && !press.sliding
                    && !press.choosing
                    && !press.cursor_started
                    && !press.gestured
            })
            .and_then(|press| press.key)
    }

    /// 这根手指在**带角标的字母键**上按够久了：开那排选项（大写 / 符号 / 小写）。
    ///
    /// 返回是否开着——按的不是这种键就不开，长按对它没有额外意义。
    /// 已经开着就返回 `true`（壳的心跳会一直来问）。
    pub fn begin_choice(&mut self, pointer: i32) -> bool {
        let Some(press) = self
            .presses
            .iter_mut()
            .find(|press| press.pointer == pointer)
        else {
            return false;
        };
        if press.choosing {
            return true;
        }
        // 只有「带角标的字母键」开着才有意义：别的键要么没有第二层含义，
        // 要么已经在走自己的手势（⌫ 上滑清空、空格拖光标）
        let Some(KeyId::Letter(_)) = press.key else {
            return false;
        };
        if press.hint.is_none() || press.sliding || press.cursor_started || press.gestured {
            return false;
        }
        press.choosing = true;
        press.choice = Chooser::DEFAULT;
        self.refresh_pressed();
        true
    }

    /// 记下这根手指已经连发过了。抬起时就不再按「点击」兑现一次——
    /// 不然按住删一串、松手那下还会多删一个，每次都要多退一格。
    pub fn note_repeat(&mut self, pointer: i32) {
        if let Some(press) = self
            .presses
            .iter_mut()
            .find(|press| press.pointer == pointer)
        {
            press.repeated = true;
        }
    }

    /// 空格上移光标的**一拍**：壳每 50ms 敲一次，问「这一拍走几格」。
    ///
    /// 速度是个小数（慢的时候几拍才够一格），用累加器摊平——不是每一拍都能走整格，
    /// 但几拍下来的总位移是对的，看着就是连续加速。
    pub fn cursor_tick(&mut self, pointer: i32) -> isize {
        let dead_zone = CURSOR_DEAD_ZONE * self.metrics.density;
        let Some(press) = self
            .presses
            .iter_mut()
            .find(|press| press.pointer == pointer)
        else {
            return 0;
        };
        if press.key != Some(KeyId::Space) {
            return 0;
        }
        press.cursor_carry += cursor_rate(press.swipe, dead_zone);
        let steps = press.cursor_carry.trunc();
        let steps = steps.clamp(-(CURSOR_MAX_STEPS as f32), CURSOR_MAX_STEPS as f32);
        press.cursor_carry -= steps;
        steps as isize
    }

    /// 命中哪个键。落在键之间的缝上、或者还没画过时是 `None`。
    fn hit(&self, x: f32, y: f32) -> Option<KeyId> {
        self.rendered
            .as_ref()
            .and_then(|keyboard| keyboard.hit(x, y))
    }

    /// 这个位置上的键，角标上写着什么字符。键没有角标（或没命中键）就是 `None`。
    ///
    /// 角标**不再是滑动手势**（2026-09-21 撤掉，见 [`Chooser`]）——现在它是长按那排的
    /// 候选（按够 `REPEAT_DELAY_MS` 时记进 [`Press::hint`]）。
    ///
    /// 角标是**布局**里的数据，不在命中矩形里——命中矩形只记「这一格是哪个键」。
    fn hint_at(&self, x: f32, y: f32) -> Option<char> {
        let id = self.hit(x, y)?;
        self.layout
            .rows()
            .iter()
            .flat_map(|row| row.keys.iter())
            .find(|key| key.id == id)
            .and_then(|key| key.hint)
    }

    /// 把「有没有键按着」记下来，只影响键帽的颜色。
    ///
    /// 多根手指同时按着时取**最后按下**的那根——键帽只画得出一个按下态，
    /// 而这已经够用：反馈要的是「我这一下碰到了」，不是同时高亮好几格。
    ///
    /// **横滑起来的那根不算**（2026-09-23）：分类标签行滑起来之后是「浏览」不是「按住了
    /// 某一格」，再按下去的样儿画（连带键帽上那个放大气泡）就与手上的动作对不上了——
    /// 用户滑标签时看到气泡跟着跑，正是这个。滑动**之前**（刚按下还没越过死区）照旧算按下。
    fn refresh_pressed(&mut self) {
        let held = self
            .presses
            .iter()
            .rev()
            .find(|press| !press.sliding && !press.scrolling_x && press.key.is_some());
        let key = held.and_then(|press| press.key);
        let clearing = held.is_some_and(|press| press.clearing);
        let deleting = held.is_some_and(|press| press.deleting);
        // 长按开着那排选项的那一根：气泡要画那一排，还得知道选中第几个
        let choice = held
            .filter(|press| press.choosing)
            .map(|press| press.choice);
        if self.pressed != key
            || self.pressed_choice != choice
            || self.pressed_clearing != clearing
            || self.pressed_deleting != deleting
        {
            self.pressed = key;
            self.pressed_choice = choice;
            self.pressed_clearing = clearing;
            self.pressed_deleting = deleting;
            self.dirty = true;
        }
    }

    /// 手指离按下那点这么近（像素）就算没挪窝。
    fn touch_slop(&self) -> f32 {
        TOUCH_SLOP * self.metrics.density
    }

    /// 当前该用的键盘主题：明暗一套，高度按屏幕高矮现算。
    fn theme(&self) -> KeyboardTheme {
        let base = if self.metrics.dark {
            KeyboardTheme::dark()
        } else {
            KeyboardTheme::light()
        };
        base.fitted(self.metrics.screen_height, self.metrics.landscape)
    }
}

impl Default for Keyboard {
    fn default() -> Self {
        Self::new()
    }
}
