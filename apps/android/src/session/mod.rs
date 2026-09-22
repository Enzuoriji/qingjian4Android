//! 一次输入法会话：持有 [`Engine`] 与自绘渲染器，把 Kotlin 侧的调用翻译成它们的方法。
//!
//! 这里只管**引擎与候选条**。键盘的画法、命中与按下状态在 [`Keyboard`] 里，
//! 触摸进来按 y 分给两边（见 [`Session::touch`]）——换掉键盘那半边不影响这一层。

mod fling;

#[cfg(test)]
mod tests;

use std::ops::Range;
use std::path::Path;

use qingjian_core::{Candidate, CandidateKind, EmojiTable, Engine, MarkedKind};
use qingjian_dictionary::Dictionary;
use qingjian_learning::{CLIPBOARD_LIMIT, EMOJI_RECENT_LIMIT, FrequencyLearner, Recent};
use qingjian_render::{
    BarHitId, BarStrip, CLIPBOARD_CELLS, FontLibrary, Frame, InputMode, KeyboardLayout, Panel,
    Preedit, PreeditSegment, PreeditStyle, RenderedBar, Renderer, Row, ShiftState, Theme,
};

use crate::action::{self, Act, Command};
use crate::error::SessionError;
use crate::keyboard::{EmojiView, Fired, Keyboard};
use crate::surface;
use crate::touch::{MotionAction, TOUCH_SLOP, within_slop};
use fling::Fling;

/// 算「滚到第几条起」时给除法的一点补偿（单位是「格」，也就是一格的万分之一）。
///
/// 见 [`Session::clipboard_first`]：不加它，滚到底时最后一格会因为浮点误差永远差一点。
const GRID_EPSILON: f32 = 1e-4;

/// 随包资源目录里的 emoji 字体名（`assets/emoji/README.md` 写了为什么要带它）。
const EMOJI_FONT: &str = "NotoColorEmoji.ttf";

/// 候选条最多铺多少格。
///
/// 引擎给的是**整份**候选，而高频音节能出几百个（`ni` 是 501 个）。候选条要**逐格量文本宽度**
/// 才能排版（走一次 cosmic-text 排版），五百格纯属白干。
///
/// 铺 80 个够滚十几屏，再往后没人看。**别再让它跟着候选数走**：引擎那边多出候选是常事。
///
/// **注意它没治好「敲 ni 卡一下」**（2026-09-21 查的）：宿主机上量过，候选条渲染一次 18µs、
/// 键盘一次 363µs，都不是大头；而同一台模拟器上「敲 n」那一拍 16ms、「敲 ni」63ms——
/// 那 4 倍差距不在渲染侧（引擎 CLI 量出来 1.47ms）。**剩下的嫌疑在跨语言那一段**
/// （取位图 → 过 JNI → Kotlin 建 Bitmap），下次查从那儿分段计时。
const CANDIDATE_LIMIT: usize = 80;

/// 表情面板那两张表（随包资源目录里解出来的，见 `assets/emoji/README.md`）。
const EMOJI_PANEL_FILE: &str = "emoji-panel.tsv";
const KAOMOJI_PANEL_FILE: &str = "kaomoji-panel.tsv";

/// 「最近用过的表情」落在数据目录里的文件名。
const EMOJI_RECENT_FILE: &str = "emoji-recent.tsv";

/// 剪贴板历史落在数据目录里的文件名（同目录下还有解出来的词库与 emoji 表）。
const CLIPBOARD_FILE: &str = "clipboard.tsv";

/// 学习数据放在数据目录下的哪个子目录。
///
/// 词频、用户词、个人 n-gram、敲错表、个人英文词都是 [`FrequencyLearner`] 的兄弟表，
/// 一次就好几张 TSV，单独一个目录免得把 `filesDir` 根上摊满。
const LEARNING_DIR: &str = "learning";

/// 学习数据的主文件名；同目录下那几张兄弟表由 [`FrequencyLearner`] 从这个名字推导出来。
///
/// 与 macOS 那边同名（`apps/macos/src/host/init.rs` 的 `load_learner`），两个壳的用户数据好对照。
const LEARNING_FILE: &str = "user.tsv";

/// 随包资源目录里的 emoji 表（中文、英文各一张，加载时合成一张）。
const EMOJI_TABLES: [&str; 2] = ["emoji-zh.tsv", "emoji-en.tsv"];

/// 返回给 Kotlin 的位掩码：哪些面变了、有没有话要交给应用。跨语言只传数字。
pub mod flags {
    /// 候选条变了，重新取位图。
    pub const BAR: i32 = 1;

    /// 键盘变了，重新取位图。
    pub const KEYBOARD: i32 = 2;

    /// 有话要交给应用（上屏文本或原样按键），取走再处理。
    pub const COMMIT: i32 = 4;

    /// 拼音行变了，`setComposingText` 镜像一次。
    pub const PREEDIT: i32 = 8;

    /// 惯性还在跑：壳接着排下一帧，问 [`Session::fling_step`] 要走多少。
    ///
    /// 这是唯一一个「下一步该做什么」的位，别的位都是「哪个面变了」——滑行得有人一直敲帧，
    /// 而帧的节拍只在壳那边（安卓有现成的 `Handler`），所以只能这么告诉它别停。
    pub const FLING: i32 = 16;
}

/// 表情面板的数据：一张表就够两种（emoji 与颜文字）。
///
/// 表是「分类 / 字符 / 名字…」几列用制表符隔开，分组用 `# group: 分类` 标出
/// （见 `assets/emoji/emoji-panel.tsv`
/// 与 `assets/kaomoji/kaomoji-panel.tsv`）。**只认前两列**——emoji 那张后头还有中英文名，
/// 面板上用不上，但留着给以后做搜索。
#[derive(Debug, Default)]
struct EmojiPanel {
    /// 分类名，顺序就是表面上的顺序。
    names: Vec<String>,

    /// 每个分类的字符，与 [`Self::names`] 一一对应。
    items: Vec<Vec<String>>,

    /// 当前选中的是第几类。
    group: usize,
}

/// 标签条一屏摆几个分类。
const EMOJI_GROUP_SLOTS: usize = 3;

/// 「最近用过的」那个分类在标签条上叫什么（它是**插在最前面**的第 0 类，不是表里的）。
const RECENT_LABEL: &str = "最近";

/// 一屏摆几个表情（与 `qingjian_render` 的 `EMOJI_COLS × EMOJI_ROWS` 是同一个数）。
const EMOJI_SLOTS: usize = 15;

/// 一行摆几格（与 `qingjian_render` 的 `EMOJI_COLS` 是同一个数）。
const EMOJI_COLS: usize = 5;

/// 一屏可见几行（与 `qingjian_render` 的 `EMOJI_ROWS` 是同一个数）。
const EMOJI_ROWS: usize = 3;

impl EmojiPanel {
    /// 从文件读。文件不在或读不了就是个空的——表情面板画不出来，别的照常用。
    fn open(path: &Path) -> Self {
        let Ok(text) = std::fs::read_to_string(path) else {
            return Self::default();
        };
        let mut names: Vec<String> = Vec::new();
        let mut items: Vec<Vec<String>> = Vec::new();
        for line in text.lines() {
            // 只认 `# group: ` 这一种注释——文件头那两行说明也是 `#` 开头，
            // 一律当分类的话面板上会冒出「由 render-panel.py 生成…」这种标签
            if let Some(name) = line.strip_prefix("# group: ") {
                names.push(name.trim().to_owned());
                items.push(Vec::new());
                continue;
            }
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let mut fields = line.split('\t');
            let (Some(_group), Some(item)) = (fields.next(), fields.next()) else {
                continue;
            };
            if let Some(last) = items.last_mut() {
                last.push(item.to_owned());
            }
        }
        // 分类名与条目要一一对应；对不上就当没读到（表被手改坏了的兜底）
        if names.len() != items.len() {
            return Self::default();
        }
        Self {
            names,
            items,
            group: 0,
        }
    }

    /// 标签条上这一屏要画的那几个分类名。
    fn labels(&self, screen: usize) -> &[String] {
        let start = (screen * EMOJI_GROUP_SLOTS).min(self.names.len());
        let end = (start + EMOJI_GROUP_SLOTS).min(self.names.len());
        &self.names[start..end]
    }

    /// 这一屏第一个标签在整份里是第几个（点标签时要把屏幕号换算回去）。
    fn screen_start(&self, screen: usize) -> usize {
        (screen * EMOJI_GROUP_SLOTS).min(self.names.len())
    }

    /// 标签条一共几屏（至少 1）。
    fn screens(&self) -> usize {
        self.names.len().div_ceil(EMOJI_GROUP_SLOTS).max(1)
    }

    /// 把「最近用过的」摆到最前面当一类（一条都没有时不摆）。
    ///
    /// emoji 与颜文字**共用一份**最近记录（搜狗那个「最近」也是不分类型的）——
    /// 刚用过的那个排第一，下次进来一眼就能点到。
    fn set_recent(&mut self, recent: &[String]) {
        let has = self.names.first().is_some_and(|name| name == RECENT_LABEL);
        if recent.is_empty() {
            if has {
                self.names.remove(0);
                self.items.remove(0);
            }
            return;
        }
        if has {
            self.items[0] = recent.to_vec();
        } else {
            self.names.insert(0, RECENT_LABEL.to_owned());
            self.items.insert(0, recent.to_vec());
        }
    }

    /// 把画不出来的条目去掉——「画不画得出来」由调用方判（那边有渲染器，知道字体里
    /// 有什么字形）。**分类被滤空了就整组去掉**：标签条上留一个点进去什么都没有的分类
    /// 没意义。
    fn retain(&mut self, keep: impl Fn(&str) -> bool) {
        let mut names = Vec::new();
        let mut items = Vec::new();
        for (name, group) in self.names.drain(..).zip(self.items.drain(..)) {
            let kept: Vec<String> = group.into_iter().filter(|item| keep(item)).collect();
            if kept.is_empty() {
                continue;
            }
            names.push(name);
            items.push(kept);
        }
        self.names = names;
        self.items = items;
        self.group = 0;
    }

    /// 当前这一类里，**从第 `first` 行起要画的那几个**。
    ///
    /// 一页放不下（笑脸那一类 172 个），所以要能往下滑——跟剪贴板那份列表同一个做法：
    /// 整行的那部分在这儿切，不足一行的零头由渲染器让开。
    fn visible(&self, first: usize) -> &[String] {
        let items = self.items.get(self.group).map_or(&[][..], Vec::as_slice);
        let start = (first * EMOJI_COLS).min(items.len());
        let end = (start + EMOJI_SLOTS).min(items.len());
        &items[start..end]
    }

    /// 当前这一类一共几行（一行的格数是 [`EMOJI_COLS`]）。
    fn rows(&self) -> usize {
        let count = self.items.get(self.group).map_or(0, Vec::len);
        count.div_ceil(EMOJI_COLS)
    }
}

/// 安卓壳持有的会话状态。
///
/// Android 一个输入法进程只服务当前前台应用，所以这里跟 macOS 一样是进程级单例，
/// 不像 Windows 要按会话分派。
///
/// 方法都必须在同一条线程上调用：里面的字体系统与字形缓存不是线程安全的，
/// 而 JNI 调用本来就都来自输入法的主线程。
pub struct Session {
    /// 输入引擎。
    engine: Engine,

    /// 自绘渲染器。字体库加载不起来时为 `None`——引擎照常能用，只是画不出键盘与候选条。
    renderer: Option<Renderer>,

    /// 输入视图的宽度（点），候选条与键盘共用，壳在尺寸变化时告知。
    width: f32,

    /// 屏幕密度（点 → 像素）。位图按它渲染，贴到屏幕上才不糊。
    density: f32,

    /// 屏幕底部被系统手势条 / 导航栏占掉的高度（点）。键要往上让开这一段。
    bottom_inset: f32,

    /// 深色主题。
    dark: bool,

    /// 横屏。转屏时壳会重新报一次，键跟着矮一截。
    landscape: bool,

    /// 键盘那台前台。`None` 表示键盘不由这里画（将来改用安卓原生控件时就是它），
    /// 位图那条路随之断掉。
    keyboard: Option<Keyboard>,

    /// Shift 在哪一档。跟着键盘走，但**引擎也要用**（英文模式决定字母大小写），所以存在会话里。
    shift: ShiftState,

    /// 中还是英。同样两边都要：键盘按键帽画字，引擎按它决定往哪条路走。
    mode: InputMode,

    /// 键盘现在在哪一页。切页只换布局，键盘本身不高不矮。
    panel: Panel,

    /// 拼音行。没在组句时为 `None`。
    preedit: Option<Preedit>,

    /// 引擎给的候选，**整份列表**。
    candidates: Vec<Candidate>,

    /// 整条候选铺开后的位置：每格在哪、多长、一共几屏。
    ///
    /// 组句一变就算一次（要量全部候选的宽度），滚动与页码都读它。见 [`BarStrip`]。
    strip: BarStrip,

    /// 候选条那条带子被拖出去多远（像素，正数 = 往后滚）。
    ///
    /// 跟着手指走。**整条带子是通的**——没有「一批一批」这回事（那是 2026-09-21 之前的做法：
    /// 一次铺 24 个再一批批翻，页码说的也是「第几批」，跟手指没对上），滚到哪儿画哪儿，
    /// 两头夹住（见 [`Self::scroll_by`]）。组句一变、或者按 `‹` `›` 翻页，都回到 0（从头看起）。
    scroll: f32,

    /// 此刻甩出去的那一段滑行（见 [`Fling`]）。手指一落下就停。
    fling: Option<Fling>,

    /// 刚才**真的滚过这条带子**的那根手指。
    ///
    /// 只有它抬起时才谈得上「甩」：抬手时每根手指都会报速度上来（点候选、敲键盘也报），
    /// 照单全收去滑带子就成了乱动。手指落下即清。
    scrolled: Option<i32>,

    /// 剪贴板历史，**最新在最前**。壳每复制一次报一条进来（[`Self::note_clipboard`]）。
    ///
    /// **落盘的**（`qingjian-learning` 的 [`Clipboard`]，数据目录里那个 `clipboard.tsv`）：
    /// 输入法进程在安卓上被杀得很勤，只在内存里的话「刚复制的那条」说没就没。
    /// 每次改动它自己就写盘，这里不用管。
    clipboard: Recent,

    /// 「最近用过的表情」，emoji 与颜文字共用一份（落盘）。
    emoji_recent: Recent,

    /// 表情面板的两份数据（emoji 与颜文字共用一套机制，各喂一份）。
    ///
    /// 走的是与剪贴板**同一套**格子：布局、命中、上屏都一样，只是喂进去的字符不同。
    emoji: EmojiPanel,
    kaomoji: EmojiPanel,

    /// 标签条翻到第几屏（一屏 [`EMOJI_GROUP_SLOTS`] 个分类）。
    emoji_group_screen: usize,

    /// 表情页的格子被拉上去多少（点）。0 是第一行贴着网格区顶边。
    ///
    /// 与剪贴板那份列表**同一套做法**：整行由会话切（[`EmojiPanel::visible`]），
    /// 不足一行的零头交给渲染器让开。一页 15 格，笑脸那一类 172 个，不滚看不完。
    emoji_scroll: f32,

    /// 剪贴板列表甩出去之后的那一段滑行（与候选条那条带子各走各的）。
    clipboard_fling: Option<Fling>,

    /// 刚才**真的滚过剪贴板列表**的那根手指（与 [`Self::scrolled`] 同一个用途）。
    clipboard_scrolled: Option<i32>,

    /// 剪贴板列表被拉上去多少（点）。0 是最新那条贴着记录区顶边。
    ///
    /// **不是「第几屏」**：列表是跟手滚的，随手停在哪儿都行——所以是个连续的位移，
    /// 由手指拖动累加（见 [`Self::scroll_clipboard`]），不是整数页号。
    clipboard_scroll: f32,

    /// 当前该画的候选条那一帧。缓冲变化或翻页时由 [`Self::refresh`] 重建。
    frame: Frame,

    /// 画好待用的候选条。
    bar: Option<RenderedBar>,

    /// 候选条脏了没有——`bar_surface` 被调用时才真重画。
    bar_dirty: bool,

    /// 拼音行变了没有——壳据此 `setComposingText` 镜像一次。
    preedit_dirty: bool,

    /// 攒着要上屏的文本。壳用 `take_commit` 取走。
    pending_commit: Option<String>,

    /// 攒着要原样交给应用的按键。壳用 `take_commands` 取走。
    pending_commands: Vec<Command>,

    /// 此刻按着的、**起手落在候选条上**的手指们，按根记。
    ///
    /// 键盘那半边的手指记在 [`Keyboard`] 自己手里，两边各记各的：一根手指属于谁，
    /// 由按下时落在哪半边决定，之后一直归它。这样抬起时不会因为手指划到了别处而丢掉这一下。
    pressed: Vec<BarPress>,
}

/// 一根按在候选条上的手指。
#[derive(Debug, Clone, Copy)]
struct BarPress {
    /// 安卓给的 pointer id。
    pointer: i32,

    /// 按下时命中的目标。落在候选之间的缝上时为 `None`。
    hit: Option<BarHitId>,

    /// 按下时的坐标。抬起时要靠它判断手指还在不在同一个目标上、划了多远。
    at: (f32, f32),

    /// 手指已经滑开了，这一下不再算「点击」。
    ///
    /// **不能直接把记录删掉**——删了抬起时就不知道刚才从哪儿按的、划了多远，
    /// 翻页手势也就跟着没了。留个标记，抬起时按「不是点击」处理，还够判是不是在划。
    sliding: bool,

    /// 上一次报上来的横坐标。横滚要的是**位移增量**（这一下比上一下挪了多少），
    /// 不是「离按下那点多远」——跟手滚就得一次一次地加。
    last: f32,
}

impl Session {
    /// 打开词库、建好引擎与渲染器。
    ///
    /// `locale` 决定中日同形字取哪家字形（`zh-CN` / `ja`）。`bundle` 是壳从 APK 里解出来的
    /// 随包资源目录，里面有 emoji 字体与 emoji 表，有哪张用哪张；`None` 表示没有（用系统的 emoji 字体、
    /// 不出 emoji 候选）。见 `assets/emoji/README.md`。
    pub fn open(
        dictionary_path: &Path,
        locale: &str,
        bundle: Option<&Path>,
        data_dir: Option<&Path>,
    ) -> Result<Self, SessionError> {
        let dictionary = Dictionary::from_path(dictionary_path)?;
        let emoji_font = bundle
            .map(|dir| dir.join(EMOJI_FONT))
            .filter(|path| path.is_file());
        let library = match emoji_font.as_deref() {
            Some(path) => FontLibrary::system_with_emoji_fonts(locale, &[path.to_path_buf()]),
            None => FontLibrary::system(locale),
        };
        let renderer = match library {
            Ok(library) => Some(Renderer::new(library)),
            Err(error) => {
                tracing::error!(%error, locale, "字体库建不起来，自绘渲染器不可用");
                None
            }
        };
        let mut engine = Engine::new(dictionary);
        if let Some(table) = bundle.and_then(load_emoji_tables) {
            tracing::info!(words = table.len(), "emoji 表已加载");
            engine = engine.with_emoji(table);
        }
        // 用户学习：不挂这个，选过的词、词频、个人 n-gram 一条都不记（引擎缺省是 `NoLearner`）。
        // 引擎那边上屏时自动记账，这里只负责把它接上、以及给它一个能落盘的地方。
        //
        // **任何一步失败都只记日志、退回不挂**：学不了顶多是排得不够顺，输入法起不来是另一回事。
        if let Some(dir) = data_dir {
            let learning = dir.join(LEARNING_DIR);
            // `write_atomic` 只写文件、**不建父目录**，所以这一步不能省——省了会一路静默失败
            // （落盘只在 `FrequencyLearner` 里打一条 warn，从外面看不出没存上）。
            match std::fs::create_dir_all(&learning) {
                Ok(()) => {
                    let path = learning.join(LEARNING_FILE);
                    match FrequencyLearner::from_path(&path) {
                        Ok(learner) => {
                            tracing::info!(path = %path.display(), entries = learner.len(), "学习数据已加载");
                            engine = engine.with_learner(Box::new(learner));
                        }
                        Err(error) => {
                            tracing::error!(path = %path.display(), %error, "学习数据读不了，这次不学习");
                        }
                    }
                }
                Err(error) => {
                    tracing::error!(path = %learning.display(), %error, "学习数据目录建不出来，这次不学习");
                }
            }
        }

        // 面板数据按**渲染器画不画得出来**过一遍：渲染器只加载清单里那几个字体、不扫系统，
        // 颜文字里那些 `⑅`、`╹`、`∀` 没有字形，摆到面板上就是一排豆腐块；
        // emoji 那边同一张表已经按字形滤过了，这里再过一道是同一条规矩。
        let mut emoji_panel = bundle.map_or_else(EmojiPanel::default, |dir| {
            EmojiPanel::open(&dir.join(EMOJI_PANEL_FILE))
        });
        let mut kaomoji_panel = bundle.map_or_else(EmojiPanel::default, |dir| {
            EmojiPanel::open(&dir.join(KAOMOJI_PANEL_FILE))
        });
        if let Some(renderer) = renderer.as_ref() {
            emoji_panel.retain(|text| renderer.covers(text));
            kaomoji_panel.retain(|text| renderer.covers(text));
        }

        // 「最近用过的」插在最前面当一类（顺序在 `retain` 之后：滤掉的那些不该再出现）
        let mut emoji_recent = data_dir.map_or_else(Recent::default, |dir| {
            Recent::open(dir.join(EMOJI_RECENT_FILE), EMOJI_RECENT_LIMIT)
        });
        // 上次用过、但这回画不出来的（换了字体之类）就别留着了
        if let Some(renderer) = renderer.as_ref() {
            emoji_recent.retain(|text| renderer.covers(text));
        }
        emoji_panel.set_recent(emoji_recent.entries());
        kaomoji_panel.set_recent(emoji_recent.entries());

        Ok(Self {
            engine,
            renderer,
            width: 0.0,
            density: 1.0,
            bottom_inset: 0.0,
            dark: false,
            landscape: false,
            keyboard: Some(Keyboard::new()),
            shift: ShiftState::default(),
            mode: InputMode::default(),
            panel: Panel::Letters,
            preedit: None,
            candidates: Vec::new(),
            strip: BarStrip::default(),
            scroll: 0.0,
            fling: None,
            scrolled: None,
            emoji_recent,
            emoji: emoji_panel,
            kaomoji: kaomoji_panel,
            emoji_group_screen: 0,
            emoji_scroll: 0.0,
            clipboard: data_dir.map_or_else(Recent::default, |dir| {
                Recent::open(dir.join(CLIPBOARD_FILE), CLIPBOARD_LIMIT)
            }),
            clipboard_fling: None,
            clipboard_scrolled: None,
            clipboard_scroll: 0.0,
            frame: Frame::default(),
            bar: None,
            bar_dirty: true,
            preedit_dirty: true,
            pending_commit: None,
            pending_commands: Vec::new(),
            pressed: Vec::new(),
        })
    }

    /// 把学习数据落盘。**壳在几个时机各调一次**：键盘窗口藏起来时（`onWindowHidden`，主路径）、
    /// 焦点离开输入框时（`onFinishInput`）、进程退出前（`onDestroy`，且必须在 `close` 之前
    /// ——`close` 一调对象就没了）、以及键盘开着时每 60 秒兜一次。
    ///
    /// **`onFinishInput` 管不了「收起键盘」**：2026-09-22 在模拟器上实测，按 BACK 收起键盘时
    /// 它压根不触发（`ImeTracker` 只报 `HIDE_SOFT_INPUT_BY_BACK_KEY`），数据最后是靠 60 秒心跳
    /// 兜下来的。`onWindowHidden` 才跟得上。
    ///
    /// **为什么不每次上屏就写**：`FrequencyLearner` 落盘是「每张脏表各写一个临时文件 + fsync + 改名」，
    /// 一次选词要写三四个文件。为学习给每一次选词压上几毫秒是设计错误
    /// （`docs/contributing.md` 的「输入优先于学习」），所以宁可丢掉最多一分钟。
    ///
    /// 没有脏数据时是空操作（各表按 dirty 位判断），所以壳不必自己记「上次存到哪儿了」。
    pub fn flush_learning(&mut self) {
        self.engine.flush_learning();
    }

    /// 清空缓冲区。换应用时壳调它，免得在 A 应用敲的拼音跑到 B 应用里。
    /// 顺带把键盘**复位回字母页**：键盘收起来再弹出来该从字母页开始（跟主流一致），
    /// 而不是上次停在数字页这次还停在那儿。这里正是那个时机——候选条上那个 ×（`Act::Clear`）
    /// 只清拼音、不动页，两者不是一回事。
    /// 返回 [`flags`] 的位掩码，跟 [`Self::touch`] 一样——**复位页之后键盘位图得重画**，
    /// 壳照那个掩码走同一套收尾（不然屏幕上还停着上一页的键）。
    pub fn clear(&mut self) -> i32 {
        self.engine.clear();
        self.set_panel(Panel::Letters);
        self.recompose();
        self.mask()
    }

    /// 键盘又要弹出来了：把页复位回字母页，返回 [`flags`] 的位掩码。
    ///
    /// 跟 [`Self::clear`] **分开**：换应用要连拼音一起丢掉，而 BACK 收起键盘再弹出来**不该动拼音**
    /// ——安卓那时根本没结束输入（`onFinishInput` 不触发），只是窗口藏了。
    pub fn reset_panel(&mut self) -> i32 {
        self.set_panel(Panel::Letters);
        self.mask()
    }

    /// 换一页：记下来，并把新布局交给键盘前台（命中矩形跟着一起换）。
    fn set_panel(&mut self, panel: Panel) {
        if panel == self.panel {
            return;
        }
        self.panel = panel;
        // 换布局要连命中矩形一起换——那两个是渲染时一起出来的
        if let Some(keyboard) = self.keyboard.as_mut() {
            keyboard.set_layout(KeyboardLayout::of(panel));
        }
        // 剪贴板页从头看起（每次进来都回到最新的那几条）
        if panel == Panel::Clipboard {
            self.clipboard_scroll = 0.0;
        }
        // 表情页也从第一屏分类看起
        if matches!(panel, Panel::Emoji | Panel::Kaomoji) {
            self.emoji_group_screen = 0;
        }
        // 换页了，正在跑的那段滑行按的是上一页的视口，停掉
        self.clipboard_fling = None;
        self.clipboard_scrolled = None;
        self.refresh();
    }

    /// 壳报告输入视图的宽度（点）、**屏幕在当前方向上的高度**（点）、屏幕密度、
    /// 底部被系统占掉的高度、明暗，
    /// 返回整块输入视图**总共该有多高**（点）：候选条 + 键盘 + 底部让开的那一段。
    ///
    /// 高度要回传是因为安卓只按视图量出来的尺寸给输入法窗口大小——壳得知道自己该占多高，
    /// 否则窗口会被撑满整屏。几项都没变时不重画。
    ///
    /// `screen_height` 只有键盘用得上（[`KeyboardTheme::fitted`] 按它定键盘高度），
    /// 候选条与它无关，所以它不进脏标记那份判断。
    pub fn configure(
        &mut self,
        width: f32,
        screen_height: f32,
        density: f32,
        bottom_inset: f32,
        dark: bool,
        landscape: bool,
    ) -> f32 {
        let density = if density > 0.0 { density } else { 1.0 };
        let bottom_inset = bottom_inset.max(0.0);
        if (self.width - width).abs() > 0.5
            || (self.density - density).abs() > 0.01
            || (self.bottom_inset - bottom_inset).abs() > 0.5
            || self.dark != dark
            || self.landscape != landscape
        {
            self.width = width;
            self.density = density;
            self.bottom_inset = bottom_inset;
            self.dark = dark;
            self.landscape = landscape;
            self.bar_dirty = true;
            // 带子的位置是**像素**——密度或主题一变就得重新铺开，不然页码按老尺寸算
            self.relayout();
            // 尺寸变了，正在跑的那段滑行按老视口算的，停掉拉倒
            self.fling = None;
        }
        if let Some(keyboard) = self.keyboard.as_mut() {
            keyboard.set_metrics(width, screen_height, density, bottom_inset, dark, landscape);
        }
        self.bar_height() + self.keyboard_height() + self.bottom_inset
    }

    /// 键盘占多高（点）。键盘不由这里画时是 0——那种情况下高度由壳自己算。
    fn keyboard_height(&self) -> f32 {
        self.keyboard.as_ref().map_or(0.0, Keyboard::height)
    }

    /// 候选条该占多高（点）。**没在组句时是 0**——这一条整个收起来，高度还给应用。
    ///
    /// 壳按两张位图的高度自己量视图，所以这个值只要跟着 [`Self::bar_surface`] 一致就行。
    pub fn bar_height(&self) -> f32 {
        Renderer::bar_height(&self.theme(), self.composing())
    }

    /// 在组句吗——拼音缓冲区里有没有东西。
    ///
    /// 候选条收不收就看它，**不是看有没有候选**：`ni'h` 这种还拼不成音节的也有拼音行要显示。
    fn composing(&self) -> bool {
        self.preedit.is_some()
    }

    /// 现在是英文模式吗。
    fn english(&self) -> bool {
        self.mode == InputMode::English
    }

    /// 当前该用的候选窗主题。
    fn theme(&self) -> Theme {
        if self.dark {
            Theme::dark()
        } else {
            Theme::light()
        }
    }

    /// 叫键盘重画（Shift 与中 / 英切换改的是键帽长相）。
    fn mark_keyboard_dirty(&mut self) {
        if let Some(keyboard) = self.keyboard.as_mut() {
            keyboard.mark_dirty();
        }
    }

    /// 候选条的位图（8 字节头 + 预乘 RGBA）。没配过宽度或渲染器不可用时返回空。
    ///
    /// **没组句时也画**：那会儿这一条细细的、里面一个标（开工具页 / 剪贴板），
    /// 一打字才被候选条整个接管（见 [`Renderer::bar_height`]）。那张位图的高度跟着变，
    /// 壳按位图高度自己量视图——所以壳那边不必知道这两种状态。
    pub fn bar_surface(&mut self) -> Vec<u8> {
        if self.width <= 0.0 {
            return Vec::new();
        }
        if self.bar_dirty || self.bar.is_none() {
            let theme = self.theme();
            // 渲染是从**传进去的第一格**开始往外铺的，所以要喂它「这一格相对视口的位置」
            let scroll = self.strip.local_scroll(self.visible().start, self.scroll);
            let rendered = self.renderer.as_mut().and_then(|renderer| {
                renderer
                    .render_bar(&self.frame, self.width, &theme, self.density, scroll)
                    .ok()
            });
            if rendered.is_none() {
                return Vec::new();
            }
            self.bar = rendered;
            self.bar_dirty = false;
        }

        self.bar
            .as_ref()
            .map_or_else(Vec::new, |bar| surface::encode(&bar.rendered.pixmap))
    }

    /// 按住键时那张预览气泡的位图（8 字节头 + 预乘 RGBA）。没在预览时是空数组。
    ///
    /// 与候选条一样，**空表示「这个小窗现在不该在」**——壳收到空字节串要把浮动小窗收起来。
    pub fn popup_surface(&mut self) -> Vec<u8> {
        let (shift, mode) = (self.shift, self.mode);
        let (first, end) = self.clipboard_range();
        let (clipboard, offset) = (
            &self.clipboard.entries()[first..end],
            self.clipboard_offset(),
        );
        let panel = match self.panel {
            Panel::Kaomoji => &self.kaomoji,
            _ => &self.emoji,
        };
        let emoji = EmojiView {
            offset: self.emoji_offset(),
            items: panel.visible(self.emoji_first_row()),
            labels: panel.labels(self.emoji_group_screen),
            group: panel
                .group
                .saturating_sub(panel.screen_start(self.emoji_group_screen)),
        };
        match self.keyboard.as_mut() {
            Some(keyboard) => keyboard.popup_surface(
                self.renderer.as_mut(),
                shift,
                mode,
                clipboard,
                offset,
                emoji,
            ),
            None => Vec::new(),
        }
    }

    /// 气泡位图**左上角**该摆在哪儿（整块输入视图的像素，与触摸坐标同一套）。
    ///
    /// 摆哪儿在这边算好告诉壳：壳只把浮动小窗挪到「视图在屏幕上的位置 + 这个偏移」，
    /// 自己不掺和布局。没在预览时是 `None`。
    pub fn popup_origin(&self) -> Option<(f32, f32)> {
        let (x, y) = self.keyboard.as_ref()?.popup_origin()?;
        // 键盘在候选条下面，加上那一段才是整块视图的坐标
        Some((x, self.bar_pixels() + y))
    }

    /// 键盘的位图（8 字节头 + 预乘 RGBA）。没配过宽度、渲染器不可用、或者键盘不由这里画时返回空。
    pub fn keyboard_surface(&mut self) -> Vec<u8> {
        let (shift, mode) = (self.shift, self.mode);
        let (first, end) = self.clipboard_range();
        let (clipboard, offset) = (
            &self.clipboard.entries()[first..end],
            self.clipboard_offset(),
        );
        let panel = match self.panel {
            Panel::Kaomoji => &self.kaomoji,
            _ => &self.emoji,
        };
        let emoji = EmojiView {
            offset: self.emoji_offset(),
            items: panel.visible(self.emoji_first_row()),
            labels: panel.labels(self.emoji_group_screen),
            group: panel
                .group
                .saturating_sub(panel.screen_start(self.emoji_group_screen)),
        };
        match self.keyboard.as_mut() {
            Some(keyboard) => keyboard.surface(
                self.renderer.as_mut(),
                shift,
                mode,
                clipboard,
                offset,
                emoji,
            ),
            None => Vec::new(),
        }
    }

    /// 该镜像给应用的拼音行，取走并清掉脏标记。
    ///
    /// 空串表示没在组句，壳应当结束组字（`finishComposingText`）——上屏之后就是这种情况。
    pub fn take_preedit(&mut self) -> String {
        self.preedit_dirty = false;
        self.frame
            .preedit
            .as_ref()
            .map(Preedit::text)
            .unwrap_or_default()
    }

    /// 取走要上屏的文本（并清掉）；`None` 表示这次没有。
    pub fn take_commit(&mut self) -> Option<String> {
        self.pending_commit.take()
    }

    /// 取走要原样交给应用的按键编号（并清掉）。编号与 [`Command::code`] 对应。
    pub fn take_commands(&mut self) -> Vec<i32> {
        self.pending_commands.drain(..).map(Command::code).collect()
    }

    /// 一次触摸（坐标是整块输入视图的）。`pointer` 是安卓给的 pointer id。返回 [`flags`] 的位掩码。
    ///
    /// 按 y 分给两边：候选条在上、键盘在下。**一根手指归谁，由按下时落在哪半边定**，
    /// 之后移动与抬起都送回同一家——手指可能已经划到另一半边上了，按当前坐标重新分派
    /// 会让这一下凭空消失。两边各自记自己那批 pointer，不认识的不理，所以这里不必再记一份归属。
    ///
    /// 键盘那边的按钮语义（**要松**：手指抖几像素不该掉字）在 [`Keyboard::touch`] 里，
    /// 候选条这边见 [`Self::touch_bar`]。
    pub fn touch(&mut self, action: MotionAction, pointer: i32, x: f32, y: f32) -> i32 {
        if matches!(action, MotionAction::Down | MotionAction::PointerDown) {
            // 手指一落下就把滑行停住：滑到一半想抓回来是「摸住就停」那个手感，
            // 不这么做的话按下去的那一下会和正在跑的惯性互相抢
            self.fling = None;
            self.scrolled = None;
            self.clipboard_fling = None;
            self.clipboard_scrolled = None;
        }
        let bar_pixels = self.bar_pixels();
        let fired = self
            .keyboard
            .as_mut()
            .and_then(|keyboard| keyboard.touch(action, pointer, x, y - bar_pixels));
        match fired {
            Some(Fired::Key(key)) => self.apply(action::on_key(key)),
            Some(Fired::MoveCursor(steps)) => self.move_cursor(steps),
            Some(Fired::ClearToStart) => self.clear_to_start(),
            Some(Fired::DeleteClipboard(index)) => self.apply(Act::DeleteClipboard(index)),
            Some(Fired::ClipboardScroll(delta)) => {
                // 记下是这根手指在滚——抬手时靠它判该不该甩（那时 `presses` 里已经没有它了）
                if matches!(self.panel, Panel::Emoji | Panel::Kaomoji) {
                    self.scroll_emoji(delta);
                } else {
                    self.clipboard_scrolled = Some(pointer);
                    self.scroll_clipboard(delta);
                }
            }
            None => {}
        }
        self.touch_bar(action, pointer, x, y);
        self.mask()
    }

    /// 光标左右移几格（正数往右）。空格键上横着滑出来的。
    ///
    /// 攒成一条条方向键交给应用，**不自己动拼音缓冲区**：输入法不知道光标前后有什么，
    /// 挪光标是应用的事（文本框 / 网页 / 代码编辑器各不一样）。
    fn move_cursor(&mut self, steps: isize) {
        // 组句当中不动光标：那会儿输入框里是我们镜像过去的拼音，挪光标该挪的是**拼音里**的位置，
        // 而引擎还没有这个能力——让应用去挪只会把组字区搅乱
        if self.composing() {
            return;
        }
        let command = if steps > 0 {
            Command::MoveRight
        } else {
            Command::MoveLeft
        };
        for _ in 0..steps.abs() {
            self.pending_commands.push(command);
        }
    }

    /// 空格上移光标的**一拍**：壳每 50ms 敲一次，返回 [`flags`] 的位掩码。
    ///
    /// 这一拍走几格由 [`Keyboard::cursor_tick`] 按位移量定（**越远越快**），
    /// 这里只负责摊成一串方向命令。壳不用自己算速度，也就不必知道死区是多少。
    pub fn cursor_tick(&mut self, pointer: i32) -> i32 {
        let steps = self
            .keyboard
            .as_mut()
            .map_or(0, |keyboard| keyboard.cursor_tick(pointer));
        if steps != 0 {
            self.move_cursor(steps);
        }
        self.mask()
    }

    /// ⌫ 上往上滑、松手：把光标前面整段清掉。
    ///
    /// 组句当中不理——那会儿输入框里是我们的拼音，要清也该清拼音（引擎那边一条命令的事），
    /// 让应用去删只会把组字区搅乱。
    fn clear_to_start(&mut self) {
        if self.composing() {
            return;
        }
        self.pending_commands.push(Command::ClearToStart);
    }

    /// 长按连发：壳的计时器到点了，问一次「按住的那个键要不要再来一次」。
    ///
    /// 计时器在壳那边（安卓有现成的 `Handler`，Rust 这边为此引线程或定时器不划算），
    /// 这里只回答**该不该触发**——哪个键连发是输入语义，不该让壳知道。
    /// 按住的键不该连发、或者那根手指已经松了，就什么也不做。
    ///
    /// **只有键盘上的键会连发**（现在就是退格）。候选条上按住不放不做任何事——
    /// 原先这儿挂着「长按候选 = 删词」，2026-09-20 摘掉了，理由见
    /// `docs/plan/android-keyboard.md` 的 K8。
    pub fn repeat(&mut self, pointer: i32) -> i32 {
        let key = self.keyboard.as_mut().and_then(|keyboard| {
            let key = keyboard.held(pointer).filter(|key| action::repeats(*key))?;
            // 记一笔，抬起时就不再按「点击」补一下了
            keyboard.note_repeat(pointer);
            Some(key)
        });
        if let Some(key) = key {
            self.apply(action::on_key(key));
            return self.mask();
        }
        // 键盘那头没按着要连发的键：**长按带角标的字母键就开那排选项**
        // （大写 / 符号 / 小写，见 `keyboard::Chooser`）。别的键按够久什么也不做。
        if let Some(keyboard) = self.keyboard.as_mut() {
            keyboard.begin_choice(pointer);
        }
        self.mask()
    }

    /// 候选条那半边。**三条路，按手指怎么动分**：
    ///
    /// - **点击**：按下再抬起、没怎么挪 → 上屏点的那个（与键盘一样的按钮语义）
    /// - **横滑**：候选条是一条**能滚的带子**，按住横着拖它就跟着手指平移
    ///   （见 [`Self::scroll_by`]）——滚过一页自动翻页。**拖动只是看，不上屏**：
    ///   想选还得点一下，或者空格上屏本页第一个
    ///
    /// 判法与键盘不同：候选格横向拖是手势，所以只按「挪没挪出触摸阈值」判，
    /// 不按「还在不在原来那一格上」判——否则拖一下会被当成点了那个候选。
    fn touch_bar(&mut self, action: MotionAction, pointer: i32, x: f32, y: f32) {
        match action {
            MotionAction::Down | MotionAction::PointerDown => {
                // 落在键盘那头，不归这里管
                if y >= self.bar_pixels() {
                    return;
                }
                let hit = self.bar.as_ref().and_then(|bar| bar.hit(x, y));
                self.pressed.retain(|held| held.pointer != pointer);
                self.pressed.push(BarPress {
                    pointer,
                    hit,
                    at: (x, y),
                    last: x,
                    sliding: false,
                });
            }
            MotionAction::Move => {
                let slop = self.touch_slop();
                let Some(held) = self.pressed.iter_mut().find(|held| held.pointer == pointer)
                else {
                    return;
                };
                if !within_slop(held.at, slop, x, y) {
                    held.sliding = true;
                }
                // 手指往左移 = 带子往左滚（看后面的候选）。**只认这一下的位移增量**，
                // 所以是跟手的：手指走多少、带子走多少。
                let step = held.last - x;
                held.last = x;
                if held.sliding && step != 0.0 {
                    self.scroll_by(step);
                    // 记下是**这根手指**在滚：只有它抬起时才谈得上甩
                    self.scrolled = Some(pointer);
                }
            }
            MotionAction::Up | MotionAction::PointerUp => {
                let index = self.pressed.iter().position(|held| held.pointer == pointer);
                let Some(ended) = index.map(|index| self.pressed.remove(index)) else {
                    return;
                };
                // 拖过就只是滚了一下，**不上屏**——选词还得点一下（或空格上屏本页第一个）。
                // 要「滚到哪儿就选哪个」的话，一路翻看过去就没法全身而退了。
                if ended.sliding {
                    return;
                }
                // 没怎么挪 = 点击：抬起时还在按下那个目标上（或只挪了触摸阈值那么点）才算数
                let hit = self.bar.as_ref().and_then(|bar| bar.hit(x, y));
                let fired = match ended.hit {
                    Some(id)
                        if hit == Some(id) || within_slop(ended.at, self.touch_slop(), x, y) =>
                    {
                        Some(id)
                    }
                    _ => None,
                };
                if let Some(id) = fired {
                    self.apply(action::on_bar(id));
                }
            }
            MotionAction::Cancel => self.pressed.clear(),
        }
    }

    /// 这一轮下来哪些面要重取、有没有话要交给应用。
    fn mask(&self) -> i32 {
        let mut mask = 0;
        if self.keyboard.as_ref().is_some_and(Keyboard::dirty) {
            mask |= flags::KEYBOARD;
        }
        if self.bar_dirty {
            mask |= flags::BAR;
        }
        if self.preedit_dirty {
            mask |= flags::PREEDIT;
        }
        if self.pending_commit.is_some() || !self.pending_commands.is_empty() {
            mask |= flags::COMMIT;
        }
        if self.fling.is_some() || self.clipboard_fling.is_some() {
            mask |= flags::FLING;
        }
        mask
    }

    /// 候选条在整块输入视图里占的高度（像素）。键盘接在它下面。
    fn bar_pixels(&self) -> f32 {
        self.bar_height() * self.density
    }

    /// 手指离按下那点这么近（像素）就算没挪窝。**要按密度换算**：安卓自己的触摸阈值是 8 dp，
    /// 直接拿 8 像素当阈值的话，密度 2.75 的机器上只有 2.9 个点，快敲必然被误判成滑动。
    fn touch_slop(&self) -> f32 {
        TOUCH_SLOP * self.density
    }

    /// 执行一个动作。引擎在什么状态决定同一动作的不同走法，都写在这里。
    fn apply(&mut self, act: Act) {
        match act {
            Act::Emoji(index) => self.commit_emoji(index),
            Act::EmojiGroup(index) => self.pick_emoji_group(index),
            Act::EmojiGroupPage(step) => self.turn_emoji_groups(step),
            Act::Push(c) => self.type_letter(c),
            Act::CommitCandidate(index) => {
                // 命中矩形里的下标是**画出来那一批**里的（从最左边看得见的那个数起）
                let absolute = self.visible().start + index;
                if let Some(candidate) = self.candidates.get(absolute).cloned() {
                    let text = self.engine.commit(&candidate);
                    self.commit_text(text);
                }
            }
            Act::CommitHighlighted => {
                // 高亮那个：就是最左边看得见的那个（空格上屏的是它）
                match self.highlighted_candidate() {
                    Some(candidate) => {
                        let text = self.engine.commit(&candidate);
                        self.commit_text(text);
                    }
                    // 没有候选可上屏，空格就是空格
                    None => {
                        self.engine.note_passthrough(' ');
                        self.commit_text(" ".to_owned());
                    }
                }
            }
            Act::CommitRaw => {
                if self.engine.composition().is_empty() {
                    self.pending_commands.push(Command::Enter);
                } else {
                    let text = self.engine.take_raw();
                    self.commit_text(text);
                }
            }
            Act::Backspace => {
                if self.engine.composition().is_empty() {
                    self.pending_commands.push(Command::Backspace);
                } else {
                    self.engine.backspace();
                    self.recompose();
                }
            }
            Act::Clear => {
                self.engine.clear();
                self.recompose();
            }
            Act::Page(step) => self.turn_page(step),
            Act::ToggleShift => {
                // 安卓上的习惯是单击锁定、再击解锁（桌面才是按住）
                self.shift = match self.shift {
                    ShiftState::Off => ShiftState::Locked,
                    _ => ShiftState::Off,
                };
                self.mark_keyboard_dirty();
            }
            Act::SwitchPanel(panel) => self.set_panel(panel),
            Act::ToggleMode => {
                self.mode = match self.mode {
                    InputMode::Chinese => InputMode::English,
                    InputMode::English => InputMode::Chinese,
                };
                // 切换时把没上屏的拼音丢掉：留着的话，英文模式下那串字母会按英文词算候选
                self.engine.clear();
                self.engine
                    .set_english_mode(self.mode == InputMode::English);
                self.mark_keyboard_dirty();
                self.recompose();
            }
            Act::Punctuate(c) => self.punctuate(c),
            Act::ToggleTools => self.toggle_tools(),
            Act::PasteClipboard(index) => self.paste_clipboard(index),
            Act::DeleteClipboard(index) => self.delete_clipboard(index),
            Act::ClearClipboard => self.clear_clipboard(),
        }
    }

    /// 打一个标点。
    ///
    /// 还在组句就先把高亮候选上屏——手机上打标点应当结束当前这串拼音，「nihao」+「，」得到
    /// 「你好，」而不是把逗号插到拼音前面去。桌面的做法是把标点收进「英文直输段」（`nihao,` 整串
    /// 一起算），那是给实体键盘的，触摸键盘上不是这个预期，这里不跟。
    fn punctuate(&mut self, c: char) {
        if !self.engine.composition().is_empty()
            && let Some(candidate) = self.highlighted_candidate()
        {
            let text = self.engine.commit(&candidate);
            self.pending_commit
                .get_or_insert_with(String::new)
                .push_str(&text);
        }
        // 英文模式打半角，不劳引擎转
        if self.english() {
            self.engine.note_passthrough(c);
            self.commit_text(c.to_string());
            return;
        }
        // 转得了全角就让引擎转（它还要记账），转不了原样打出去并告知
        let text = match self.engine.punctuate(c) {
            Some(text) => text.to_owned(),
            None => {
                self.engine.note_passthrough(c);
                c.to_string()
            }
        };
        self.commit_text(text);
    }

    /// 敲一个字母。
    ///
    /// 中文模式：进组句缓冲区，大小写不影响拼音（Shift 只改键帽）。
    ///
    /// 英文模式：**直输**，字母不进缓冲区、直接打给应用，大小写跟着 Shift 走。
    /// 引擎的英文候选要另外喂一张英文词表（`Engine::with_english`），安卓这边还没随包带，
    /// 所以给不了候选——这也是别的壳关掉英文候选时走的那条路。要接候选得先把英文词表生成出来。
    fn type_letter(&mut self, c: char) {
        if self.english() {
            let c = if self.shift.is_upper() {
                c.to_ascii_uppercase()
            } else {
                c.to_ascii_lowercase()
            };
            self.engine.note_passthrough(c);
            self.commit_text(c.to_string());
        } else {
            self.engine.push(c.to_ascii_lowercase());
            self.recompose();
        }
    }

    /// 攒下要上屏的文本。
    ///
    /// 上屏之后要重查：候选比输入短时（`kaifazhe` 选了「开发」）剩下的拼音还在缓冲区里，
    /// 得接着给候选；整段吃完时缓冲空掉，候选条自然清干净。
    fn commit_text(&mut self, text: String) {
        self.pending_commit
            .get_or_insert_with(String::new)
            .push_str(&text);
        self.recompose();
    }

    /// 缓冲变了：重查候选，带子从头铺开、也从头上看起。
    fn recompose(&mut self) {
        self.scroll = 0.0;
        // 滑行是从「刚才滚到哪儿」接着走的，候选一换就没意义了
        self.fling = None;
        self.scrolled = None;
        self.candidates.clear();
        self.preedit = None;
        if !self.engine.composition().is_empty() {
            match self.engine.query() {
                Ok(query) => {
                    let segments = query.marked_segments();
                    self.preedit = Some(Preedit {
                        segments: segments.iter().map(preedit_segment).collect(),
                        cursor: query.marked_cursor(),
                    });
                    self.candidates = query.candidates.items;
                }
                Err(_) => {
                    // 还拼不成拼音：拼音行照画，只是没有候选
                    let text = self.engine.composition().text().to_owned();
                    let cursor = text.chars().count();
                    self.preedit = Some(Preedit::plain(&text, cursor));
                }
            }
        }
        // 候选整份换了，带子得重铺——**在 build_frame 之前**，画哪几格要读它
        self.relayout();
        self.refresh();
        self.preedit_dirty = true;
    }

    /// 用当前的候选与滚动位置重建待画的那一帧。
    ///
    /// 滚动与翻页走这条路而不是 [`Self::recompose`]——重查会把带子打回头重看。
    fn refresh(&mut self) {
        self.frame = self.build_frame();
        self.bar_dirty = true;
    }

    /// 视口有多宽（像素）：候选条是通栏的，就是输入视图那么宽。
    fn viewport(&self) -> f32 {
        self.width * self.density
    }

    /// 此刻该画哪几格（左闭右开）。
    fn visible(&self) -> Range<usize> {
        self.strip.slice(self.scroll, self.viewport())
    }

    /// 画哪几个候选、页码是几。这一轮不画译文，所以不调 `engine.annotate()`。
    ///
    /// **只画看得见的那几格**（十来个，两边各带一个只露半边的），不是「铺一整批」——
    /// 带子是通的，滚到哪儿画哪儿。
    fn build_frame(&self) -> Frame {
        let visible = self.visible();
        let rows: Vec<Row> = self.candidates[visible.clone()]
            .iter()
            .enumerate()
            .map(|(i, candidate)| row(visible.start + i, candidate))
            .collect();
        let viewport = self.viewport();
        let screens = self.strip.screens(viewport);
        Frame {
            preedit: self.preedit.clone(),
            // 高亮**跟着带子走**：压在最左边那个**整格**上，空格上屏的就是它。
            // 认整格不认「露了半边的那个」——压在半格上的话，看着像画坏了，
            // 而且空格上屏的会是屏幕上几乎看不见的词。
            highlighted: self.highlighted_index().map(|index| index - visible.start),
            footer: self.footer(screens),
            rows,
            sentence: None,
            status: None,
        }
    }

    /// 这一帧右上角那个页码写什么（没有就 `None`）。
    ///
    /// **只有候选翻页才报**：剪贴板列表 2026-09-21 改成跟手滚动之后就没有「第几屏」了
    /// （要滚到哪儿看手指，不是一个页号说得清的）。
    /// 只有一屏也不报。
    fn footer(&self, candidate_screens: usize) -> Option<String> {
        (candidate_screens > 1).then(|| {
            format!(
                "{}/{}",
                self.strip.screen(self.scroll, self.viewport()),
                candidate_screens
            )
        })
    }

    /// 高亮那个候选在整份候选表里排第几：**最左边那个整格**。一个候选都没有时是 `None`。
    ///
    /// 夹进这一屏画出来的那一段里：滚到带子中间时，第一个整格理论上还在可见范围里，
    /// 但两头（尤其贴到底那一下）可能落到外面去。
    fn highlighted_index(&self) -> Option<usize> {
        let visible = self.visible();
        if visible.is_empty() {
            return None;
        }
        Some(
            self.strip
                .first_whole(self.scroll)
                .clamp(visible.start, visible.end - 1),
        )
    }

    /// 高亮那个候选。空格上屏、组句中打标点先上屏，走的都是它——
    /// 所以「高亮在哪」只有这一个说法。
    fn highlighted_candidate(&self) -> Option<Candidate> {
        self.highlighted_index()
            .and_then(|index| self.candidates.get(index))
            .cloned()
    }

    /// 按当前候选把带子重铺一遍（要量全部候选的宽度，见 [`BarStrip`]）。
    ///
    /// 组句一变就得重铺（候选整份换了）；密度或明暗变了也得（带子的位置是像素算的，
    /// 字号跟着主题走）。渲染器不可用时给空的那份——那时候候选条本来就画不出来。
    ///
    /// **只铺前 [`CANDIDATE_LIMIT`] 个**：这一步要逐格量宽度，引擎那边动不动几百个候选，
    /// 全铺下来就是几十毫秒（见那个常数的注释）。
    fn relayout(&mut self) {
        let rows: Vec<Row> = self
            .candidates
            .iter()
            .take(CANDIDATE_LIMIT)
            .enumerate()
            .map(|(i, candidate)| row(i, candidate))
            .collect();
        let theme = self.theme();
        self.strip = self
            .renderer
            .as_mut()
            .map(|renderer| renderer.bar_strip(&rows, &theme, self.density))
            .unwrap_or_default();
    }

    /// 翻页：往前后挪 `step` 屏（`‹` `›` 两个箭头走这里）。
    ///
    /// **一屏一屏地停**：先把当前位置折到屏边界上再挪——手指拖到半屏的地方按一下，
    /// 只该挪到下一屏的边界，不该只挪半个屏（那样页码也不动，看着像没反应）。
    /// 带子一屏就装得下时不动，也不记账。
    fn turn_page(&mut self, step: isize) {
        let viewport = self.viewport();
        if viewport <= 0.0 {
            return;
        }
        let screen = self.strip.screen(self.scroll, viewport) as isize;
        let target = (screen + step).clamp(1, self.strip.screens(viewport) as isize) as usize;
        let wanted = self.strip.screen_scroll(target, viewport);
        if wanted != self.scroll {
            self.scroll = wanted;
            self.engine.note_page_turn();
            self.refresh();
        }
    }

    /// 候选条那条带子横滚 `step` 像素（正数 = 往后滚，看后面的候选）。
    ///
    /// 就这么简单：**带子是通的**，滚到哪儿算哪儿，两头夹住——滚到头再拖也拖不出空白。
    /// 早先这里还要判「滚过一页就翻页」，那是因为一次只铺 24 个；现在整条铺开、
    /// 画哪几格由 [`Self::visible`] 现算，翻页这件事就不存在了。
    fn scroll_by(&mut self, step: f32) {
        let max = self.strip.max_scroll(self.viewport());
        let wanted = (self.scroll + step).clamp(0.0, max);
        if wanted == self.scroll {
            return;
        }
        self.scroll = wanted;
        self.refresh();
    }

    /// 一根手指抬起了：报上它的速度（**像素/秒**，横向向右为正、纵向向下为正，
    /// 都是 `VelocityTracker` 的单位与方向），够快就让刚才滚的那个接着滑一段。
    ///
    /// **速度由壳量**（安卓自带 `VelocityTracker`，自己算得再去摸时间戳），
    /// 甩不甩、甩多远由这里定（[`Fling`]）。只有刚才**真的滚过**的那根手指才算数——
    /// 点候选、敲键盘时壳同样会报速度上来，那不是「甩」。
    ///
    /// 横竖两个分量是**两回事**：候选条那条带子横着滚（用 [`Self::scroll_by`] 的方向），
    /// 剪贴板列表竖着滚（用 [`Self::scroll_clipboard`] 的方向，符号正好相反）。
    pub fn start_fling(&mut self, pointer: i32, velocity_x: f32, velocity_y: f32) -> i32 {
        if self.clipboard_scrolled == Some(pointer) {
            // 手指往上甩（速度为负）= 列表往下看 = `clipboard_scroll` 变大，
            // 而 `scroll_clipboard` 收的是「手指挪了多少」，所以符号**不取反**；
            // 除以 1000 换成 `Fling` 用的像素/毫秒
            self.clipboard_fling = Fling::new(velocity_y / 1000.0);
        } else if self.scrolled == Some(pointer) {
            // 手指往左甩（速度为负）= 带子往后滚，与 `scroll_by` 的正方向一致，所以取负
            self.fling = Fling::new(-velocity_x / 1000.0);
        }
        self.mask()
    }

    /// 壳复制到东西了：记一条进历史，**最新的在最前**。
    ///
    /// 同一条文本再复制一次不重复记，只把它挪到最前——「刚复制的永远第一条」。
    /// 超过 [`CLIPBOARD_LIMIT`] 条丢最旧的。空白的壳那边就滤掉了，这里再挡一道。
    pub fn note_clipboard(&mut self, text: &str) -> i32 {
        if self.clipboard.remember(text) {
            self.after_clipboard_change();
        }
        self.mask()
    }

    /// 剪贴板那份列表变了：剪贴板页开着就重画（画的是那份列表）。
    ///
    /// 删到没那么多条了可能就滚过头了，先夹回来——不然会停在一段空白上。
    fn after_clipboard_change(&mut self) {
        // 列表都变了，正在跑的那段滑行按的是老列表，停掉
        self.clipboard_fling = None;
        self.clipboard_scroll = self
            .clipboard_scroll
            .clamp(0.0, self.clipboard_max_scroll());
        if self.panel == Panel::Clipboard {
            self.mark_keyboard_dirty();
            self.refresh();
        }
    }

    /// 剪贴板列表此刻该画的几条，在整份里的下标范围。
    ///
    /// **整格的部分在这儿切好**：滚动量落在一格中间时，那点零头由渲染器让开
    /// （见 `Renderer::render_keyboard` 里的 `frac`），这儿只管从第几条起。
    ///
    /// 返回的是两个数而不是切片——切片借的是 `self`，调用处还要同时借
    /// `self.keyboard` / `self.renderer`（可变），借不到一块儿去。
    fn clipboard_range(&self) -> (usize, usize) {
        let len = self.clipboard.len();
        let first = self.clipboard_first().min(len);
        (first, (first + CLIPBOARD_CELLS).min(len))
    }

    /// 列表顶边现在对着整份里的第几条。
    fn clipboard_first(&self) -> usize {
        let pitch = self.grid_pitch();
        if pitch <= 0.0 {
            return 0;
        }
        // 除以一格的高度取整，**加一点补偿**：滚动量是一格格累加、又夹在
        // 「多出来的格数 × 一格高」上的，而浮点下 `3 × pitch ÷ pitch` 可能算出 2.99999…，
        // floor 之后就少一格——滚到底时最后一格会永远差一丁点露不全。
        // 补偿取一格的万分之一，比任何有意义的手势位移都小。
        let grids = self.clipboard_scroll / pitch + GRID_EPSILON;
        grids.floor().max(0.0) as usize
    }

    /// 整格之外还让开了多少（点，0 到一格高之间）——渲染器只拿它把卡片平移一下。
    ///
    /// 给的是**余量**不是滚动总量：整格那部分已经在 [`Self::clipboard_first`] 里换成了
    /// 「从第几条起」，渲染器不必（也不该）再做一次除法。
    fn clipboard_offset(&self) -> f32 {
        let pitch = self.grid_pitch();
        if pitch <= 0.0 {
            return 0.0;
        }
        let frac = self.clipboard_scroll - self.clipboard_first() as f32 * pitch;
        frac.clamp(0.0, pitch)
    }

    /// 剪贴板列表还能往下滚多少（点）：比一屏多出来的那几条，一条一格。
    fn clipboard_max_scroll(&self) -> f32 {
        self.clipboard.len().saturating_sub(CLIPBOARD_CELLS) as f32 * self.grid_pitch()
    }

    /// 键盘一行有多高（含行间那条缝）。剪贴板「一格」与表情页「一行」都是它。
    fn grid_pitch(&self) -> f32 {
        self.keyboard
            .as_ref()
            .map_or(0.0, Keyboard::clipboard_pitch)
    }

    /// 当前这一页该用哪份表情数据：颜文字页用颜文字那份，其余（只有表情页）用 emoji 那份。
    fn emoji_panel(&self) -> &EmojiPanel {
        match self.panel {
            Panel::Kaomoji => &self.kaomoji,
            _ => &self.emoji,
        }
    }

    /// 同上，要改的时候用这个。
    fn emoji_panel_mut(&mut self) -> &mut EmojiPanel {
        match self.panel {
            Panel::Kaomoji => &mut self.kaomoji,
            _ => &mut self.emoji,
        }
    }

    /// 表情页点了一个：**整条上屏**（不是像打字那样一个字符一个字符喂给引擎）。
    ///
    /// 上屏之后**留在这一页**——发 emoji 常常一次发好几个，弹回字母页反而要重新点进来。
    fn commit_emoji(&mut self, index: usize) {
        let first = self.emoji_first_row();
        let Some(text) = self.emoji_panel().visible(first).get(index).cloned() else {
            return;
        };
        self.commit_text(text.clone());
        // 用过就记进「最近」，下次进来排在最前面（两页共用一份，所以两页都要更新）
        if self.emoji_recent.remember(&text) {
            let recent = self.emoji_recent.entries().to_vec();
            self.emoji.set_recent(&recent);
            self.kaomoji.set_recent(&recent);
            self.mark_keyboard_dirty();
        }
    }

    /// 点了分类标签：切到那一类。`index` 是**这一屏里的第几个**，要换算回整份里的下标。
    fn pick_emoji_group(&mut self, index: usize) {
        let start = self.emoji_panel().screen_start(self.emoji_group_screen);
        let target = start + index;
        let panel = self.emoji_panel_mut();
        if target >= panel.names.len() {
            return;
        }
        panel.group = target;
        // 换了一类就从上头看起（不然会停在上一次滑到的位置，看着像空的）
        self.emoji_scroll = 0.0;
        self.mark_keyboard_dirty();
    }

    /// 表情页的格子滚到第几行起了。
    fn emoji_first_row(&self) -> usize {
        let pitch = self.grid_pitch();
        if pitch <= 0.0 {
            return 0;
        }
        // 与剪贴板同一个理由：浮点下 `n × pitch ÷ pitch` 可能是 n-0.00001，
        // floor 之后就少一行，滑到底时最后一行永远差一点露不全
        let rows = self.emoji_scroll / pitch + GRID_EPSILON;
        rows.floor().max(0.0) as usize
    }

    /// 整行之外还让开了多少（点，0 到一行高之间）——渲染器拿它把整片格子平移。
    fn emoji_offset(&self) -> f32 {
        let pitch = self.grid_pitch();
        if pitch <= 0.0 {
            return 0.0;
        }
        let frac = self.emoji_scroll - self.emoji_first_row() as f32 * pitch;
        frac.clamp(0.0, pitch)
    }

    /// 表情页还能往下滚多少（点）：比一屏多出来的那几行，一行一格。
    fn emoji_max_scroll(&self) -> f32 {
        let visible = EMOJI_ROWS;
        self.emoji_panel().rows().saturating_sub(visible) as f32 * self.grid_pitch()
    }

    /// 表情页的格子跟着手指滚（`delta` 是这一拍挪了多少**像素**，往下为正）。
    fn scroll_emoji(&mut self, delta: f32) {
        let density = if self.density > 0.0 {
            self.density
        } else {
            1.0
        };
        let next = self.emoji_scroll - delta / density;
        self.emoji_scroll = next.clamp(0.0, self.emoji_max_scroll());
        self.mark_keyboard_dirty();
    }

    /// 标签条往前后翻一屏，夹在首末屏之间。
    fn turn_emoji_groups(&mut self, step: isize) {
        let screens = self.emoji_panel().screens();
        let target = (self.emoji_group_screen as isize + step).clamp(0, screens as isize - 1);
        let target = target as usize;
        if target == self.emoji_group_screen {
            return;
        }
        self.emoji_group_screen = target;
        self.mark_keyboard_dirty();
    }

    /// 剪贴板列表跟着手指滚。
    ///
    /// `delta` 是手指这一拍挪了多少**像素**（往下为正）——手指往下拖是把内容往下带，
    /// 看的是更前面的条目，所以滚动量减。夹在 `[0, 还能滚多少]` 里。
    fn scroll_clipboard(&mut self, delta: f32) {
        let density = if self.density > 0.0 {
            self.density
        } else {
            1.0
        };
        let next = self.clipboard_scroll - delta / density;
        self.clipboard_scroll = next.clamp(0.0, self.clipboard_max_scroll());
        self.mark_keyboard_dirty();
        self.refresh();
    }

    /// 标：开 / 收工具页。已经在工具页或剪贴板页时收回字母页。
    ///
    /// 仿搜狗那个 S 的开关手感：同一个按钮管开也管收。工具页是这些「不是打字的」页的入口，
    /// 以后的震动 / 设置都排在那儿。
    fn toggle_tools(&mut self) {
        let panel = match self.panel {
            // 这四页都是「不是打字的」子页：点标一律收回字母页
            Panel::Tools | Panel::Clipboard | Panel::Emoji | Panel::Kaomoji => Panel::Letters,
            _ => Panel::Tools,
        };
        self.set_panel(panel);
    }

    /// 把剪贴板里第 `index` 格（**屏幕上那一格**）那条插到光标处。
    ///
    /// 走 [`Self::commit_text`]，也就是壳那边一句 `commitText`——**插在光标处**，
    /// 正是「点一条就插进来」那个用法。插完收回字母页：粘完就该接着打字了
    /// （fcitx5 那个 `clipboardReturnAfterPaste` 是同一个意思，它做成了开关）。
    fn paste_clipboard(&mut self, index: usize) {
        let Some(entry) = self.clipboard_entry(index).cloned() else {
            return;
        };
        self.commit_text(entry);
        self.set_panel(Panel::Letters);
    }

    /// 删掉剪贴板里第 `index` 格（**屏幕上那一格**）那条。在记录上往左滑、松手走这条。
    fn delete_clipboard(&mut self, index: usize) {
        let Some(index) = self.clipboard_first().checked_add(index) else {
            return;
        };
        if self.clipboard.remove(index) {
            self.after_clipboard_change();
        }
    }

    /// 屏幕上第 `index` 格对着整份里的哪一条。滚到头、这一格没内容时是 `None`。
    fn clipboard_entry(&self, index: usize) -> Option<&String> {
        self.clipboard.entries().get(self.clipboard_first() + index)
    }

    /// 清空整份剪贴板历史。
    fn clear_clipboard(&mut self) {
        if self.clipboard.clear() {
            self.after_clipboard_change();
        }
    }

    /// 惯性的一拍：过去 `dt` 毫秒，这一拍该挪多少由 [`Fling`] 算。
    ///
    /// 节拍在壳（安卓有现成的 `Handler`，还有真帧率）、手感在这——与长按连发、移光标同一个分工。
    /// 滚到头、或者慢到看不出在动，就停（掩码里不再有 [`flags::FLING`]，壳那边跟着不再敲帧）。
    pub fn fling_step(&mut self, dt: f32) -> i32 {
        // 候选条那条带子（横着滚）
        if let Some(fling) = self.fling.as_mut() {
            let step = fling.step(dt);
            let finished = fling.finished();
            let before = self.scroll;
            self.scroll_by(step);
            // 「想走却一步没动」= 滚到头了，停；`step` 本来就是 0 的（这一拍没时间）不算
            if finished || (step != 0.0 && self.scroll == before) {
                self.fling = None;
            }
        }
        // 剪贴板那份列表（竖着滚）。两套各记各的速度，但只有一套会在跑——
        // 剪贴板页开着时没有候选条，反过来也一样
        if let Some(fling) = self.clipboard_fling.as_mut() {
            let step = fling.step(dt);
            let finished = fling.finished();
            let before = self.clipboard_scroll;
            self.scroll_clipboard(step);
            if finished || (step != 0.0 && self.clipboard_scroll == before) {
                self.clipboard_fling = None;
            }
        }
        self.mask()
    }
}

/// 把随包目录里的 emoji 表合成一张；一张都没有返回 `None`，坏了的记日志跳过。
///
/// 与 macOS 壳 `host/init.rs` 的 `load_emoji_tables` 同一套做法（中文表与英文表各配 emoji，合起来用）。
fn load_emoji_tables(dir: &Path) -> Option<EmojiTable> {
    let mut merged: Option<EmojiTable> = None;
    for name in EMOJI_TABLES {
        let path = dir.join(name);
        if !path.is_file() {
            continue;
        }
        match EmojiTable::from_path(&path) {
            Ok(table) => match &mut merged {
                Some(all) => all.merge(table),
                None => merged = Some(table),
            },
            Err(error) => tracing::warn!(%error, name, "emoji 表加载失败，跳过"),
        }
    }
    merged
}

/// 候选 → 渲染器的一行，`index` 是页内下标。
///
/// 译文这一轮不画（也就没调 `engine.annotate()`），所以只填序号与词。
/// 接译文时照 `apps/windows/server/src/ui/candidates/row.rs` 的 `from_candidate` 补上 annotation。
fn row(index: usize, candidate: &Candidate) -> Row {
    let mut row = Row::plain(index, candidate.text.as_str());
    row.cloud = candidate.kind == CandidateKind::Cloud;
    row
}

/// 引擎的 marked 分段 → 渲染器的拼音分段。
fn preedit_segment(segment: &qingjian_core::MarkedSegment) -> PreeditSegment {
    PreeditSegment {
        text: segment.text.clone(),
        style: match segment.kind {
            MarkedKind::Typed => PreeditStyle::Typed,
            MarkedKind::Rest => PreeditStyle::Rest,
            // 纠错里被改掉的原字母画删除线
            MarkedKind::Corrected => PreeditStyle::Struck,
        },
    }
}
