//! 一次输入法会话：持有 [`Engine`] 与自绘渲染器，把 Kotlin 侧的调用翻译成它们的方法。
//!
//! 这里只管**引擎与候选条**。键盘的画法、命中与按下状态在 [`Keyboard`] 里，
//! 触摸进来按 y 分给两边（见 [`Session::touch`]）——换掉键盘那半边不影响这一层。

mod config;
mod fling;
mod slide;

#[cfg(test)]
mod tests;

use std::ops::Range;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use qingjian_core::{
    Candidate, CandidateKind, CandidateLayout, Cell, CloudWord, EmojiTable, Engine, Language,
    MarkedKind, SurroundingText,
};
use qingjian_dictionary::{Dictionary, WordList};
use qingjian_learning::{CLIPBOARD_LIMIT, EMOJI_RECENT_LIMIT, FrequencyLearner, Recent};
use qingjian_lm::BigramModel;
use qingjian_platform::extra_dictionaries;
use qingjian_render::{
    BarHitId, BarStrip, CLIPBOARD_CELLS, FontLibrary, Frame, GroupIcon, GroupLabel, InputMode,
    KeyboardLayout, Panel, PanelArea, Preedit, PreeditSegment, PreeditStyle, RenderedBar,
    RenderedPanel, Renderer, Row, ShiftState, Theme, Tone,
};
use qingjian_translate::Glossary;

use crate::action::{self, Act, Command};
use crate::error::SessionError;
use crate::keyboard::{ClipboardView, EmojiView, Fired, Keyboard};
use crate::surface;
use crate::touch::{MotionAction, TOUCH_SLOP, within_slop};
use config::{ConfigState, attach_cloud, configured_language, push_to_engine};
use fling::Fling;
use slide::Slide;

/// 算「滚到第几条起」时给除法的一点补偿（单位是「格」，也就是一格的万分之一）。
///
/// 见 [`Session::clipboard_first`]：不加它，滚到底时最后一格会因为浮点误差永远差一点。
const GRID_EPSILON: f32 = 1e-4;

/// 随包资源目录里的 emoji 字体名（`assets/emoji/README.md` 写了为什么要带它）。
const EMOJI_FONT: &str = "NotoColorEmoji.ttf";

/// 等云联想结果的上限。
///
/// 过了就别等了——**不预留、不画占位**（`docs/design/candidate-ui.md` 承诺过），
/// 结果永远不来也只是这一轮没有变化。与 mac 的 `PredictMonitor::MAX_WAIT` 同一个量级。
const PREDICTION_WAIT: Duration = Duration::from_secs(12);

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

/// 随包资源目录里的英文词表。挂上它英文模式才有补全与拼错纠正；
/// 没有就退回直输（字母直接打给应用），见 [`Session::english_candidates`]。
const ENGLISH_FILE: &str = "english.tsv";

/// 随包资源目录里的语言模型（二元）。整句转换靠它；没有就退化成一元词频整句。
///
/// **它是包里最大的一件**（44 MB），值不值得带是量过的——长句上首选命中 37.5% → 62.5%，
/// 见 `docs/plan/android-engine.md` 的 E3。
const LANGUAGE_MODEL_FILE: &str = "lm.qj";

/// 英→中那本释义表（英文模式的候选靠它）。
const GLOSSARY_ENGLISH_FILE: &str = "glossary-zh.qj";

/// 随包资源目录里领域词库的子目录（成语 / 医学 / 法律 / 地名 …，一个目录好几本 `.qj`）。
///
/// 设置页也要用它（列词库清单的桥在 `crate::settings`），所以是 `pub(crate)`。
pub(crate) const DICTS_DIR: &str = "dicts";

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

    /// 用户点了工具页的「设置」：壳把键盘收起来、打开设置页（`take_settings` 取走这笔账）。
    ///
    /// 与 [`Self::FLING`] 一样是「下一步该做什么」——开会话碰不到安卓的窗口系统，
    /// 只能这么告诉壳。
    pub const SETTINGS: i32 = 32;

    /// 云联想有请求在飞：壳按拍子问 `poll_prediction` 取结果。
    ///
    /// 与 [`Self::FLING`] 一样是「下一步该做什么」——结果是**非阻塞取的**，得有人一直问。
    /// **只在真有请求在飞时才有这一位**：平时一个定时器都不跑（E7 定的那条）。
    pub const PREDICTING: i32 = 64;
}

/// 表情面板的数据：一张表就够两种（emoji 与颜文字）。
///
/// 表是「分类 / 字符 / 名字…」几列用制表符隔开，分组用 `# group: 分类` 标出
/// （见 `assets/emoji/emoji-panel.tsv`
/// 与 `assets/kaomoji/kaomoji-panel.tsv`）。**只认前两列**——emoji 那张后头还有中英文名，
/// 面板上用不上，但留着给以后做搜索。
#[derive(Debug)]
struct EmojiPanel {
    /// 一页摆几个（表情 15、颜文字 20——颜文字那页没有标签行，多一行格子）。
    slots: usize,

    /// 「最近」一条都没有时，留不留它那一类。
    ///
    /// 表情页**不留**：没东西就不摆那一格，标签行上干干净净（少一格，别的分类还宽一点）。
    /// 颜文字页**留**：右上角那个「最近」按钮要一直在，而且它按的**下标**得一直是 0——
    /// 不然按钮会随着「有没有用过」在「最近」和「全部」之间变意思。
    sticky_recent: bool,

    /// 分类名，顺序就是表面上的顺序。
    names: Vec<String>,

    /// 每个分类的字符，与 [`Self::names`] 一一对应。
    items: Vec<Vec<String>>,

    /// 所有分类的条目按 [`EMOJI_SLOTS`] 个一页切开、首尾相接——一页就是屏幕上摆得下的
    /// 那一整屏，**表情页横着翻的就是它**（2026-09-23 起；原来是一个分类竖着滚到底）。
    pages: Vec<Vec<String>>,

    /// 第 i 个分类占 [`Self::pages`] 的哪几页（闭区间），与 [`Self::names`] 一一对应。
    ranges: Vec<(usize, usize)>,

    /// 标签行上那一排：与 [`Self::names`] 一一对应，说了每一格画图标还是画名字。
    ///
    /// **跟着 [`Self::names`] 一起算好存着**（不是每次现算）：它里面存了名字的副本，
    /// 现算的话借的正是 `names`，一个结构借自己存不下来。
    labels: Vec<GroupLabel>,
}

/// 「最近用过的」那个分类在标签条上叫什么（它是**插在最前面**的第 0 类，不是表里的）。
const RECENT_LABEL: &str = "最近";

/// **一页摆几个**表情——一屏摆得下多少就是一页多少（`qingjian_render` 的
/// `EMOJI_COLS × EMOJI_ROWS`，两处是同一个数）。横着翻页翻的单位就是它。
const EMOJI_SLOTS: usize = 15;

/// 颜文字**一页摆几个**——那一页没有分类标签行，格子比表情页多一行
/// （`EMOJI_COLS × KAOMOJI_ROWS` = 20 格），但**第一格被「最近」占了**，
/// 所以能摆的颜文字是 19 个（2026-09-23：用户要求「最近固定第一个、颜文字往它右面排」）。
const KAOMOJI_SLOTS: usize = 20 - 1;

/// 「清空」的第一下之后，多久之内点第二下才算确认（超过就退回原样）。
///
/// 5 秒：够看清键帽上那句「确认清空」再点，又不至于放着很久还挂着待确认。
const CLEAR_CONFIRM_WINDOW: Duration = Duration::from_secs(5);

/// 手速超过这么多（**点/毫秒**）就算「甩」，朝手甩的方向翻一页，跟拖了多远无关。
///
/// 0.05 点/毫秒 = 50 点/秒，正是安卓 `ViewConfiguration` 的 `scaledMinimumFlingVelocity`
/// （50 dp/s）——fcitx5-android 那个 ViewPager2 判「甩没甩」用的就是这条线。
/// **特别低**：轻轻一拨就过线，所以「滑一下就翻页」。只判方向、不看大小（再快也只翻一页）。
const MIN_PAGE_FLING: f32 = 0.05;

impl Default for EmojiPanel {
    /// 一页的大小给表情页那个数：**默认值不能是 0**——`rebuild` 按它切页，
    /// 0（或者没设过）会切成「一页一个」，几十个条目就是几十页。
    fn default() -> Self {
        Self {
            slots: EMOJI_SLOTS,
            sticky_recent: false,
            names: Vec::new(),
            items: Vec::new(),
            pages: Vec::new(),
            ranges: Vec::new(),
            labels: Vec::new(),
        }
    }
}

impl EmojiPanel {
    /// 从文件读。文件不在或读不了就是个空的——表情面板画不出来，别的照常用。
    fn open(path: &Path, slots: usize, sticky_recent: bool) -> Self {
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
        let mut panel = Self {
            slots,
            sticky_recent,
            names,
            items,
            ..Self::default()
        };
        panel.rebuild();
        panel
    }

    /// 把每个分类的条目按 [`EMOJI_SLOTS`] 个一页切开，重排页区间。
    ///
    /// **内容变过就要重来一遍**（「最近」多了一条 / 少了、滤掉画不出来的字形），
    /// 页数与每一页的起点都会跟着变。切完把当前页夹回范围内：页数会变少，
    /// 不夹的话下标指到外面去，那一页就是空的。
    fn rebuild(&mut self) {
        self.pages.clear();
        self.ranges.clear();
        for items in &self.items {
            let first = self.pages.len();
            for chunk in items.chunks(self.slots.max(1)) {
                self.pages.push(chunk.to_vec());
            }
            // 一个条目都没有的分类给一个**空页**占位：页区间不能是空的，
            // 空了「点这个分类跳它第一页」会跳到隔壁分类去
            if self.pages.len() == first {
                self.pages.push(Vec::new());
            }
            self.ranges.push((first, self.pages.len() - 1));
        }
        self.labels = self.names.iter().map(|name| group_label(name)).collect();
    }

    /// 第 `page` 页属于第几个分类。
    fn group_of_page(&self, page: usize) -> usize {
        self.ranges
            .iter()
            .position(|(first, last)| (*first..=*last).contains(&page))
            .unwrap_or(0)
    }

    /// 第 `group` 个分类的第一页是哪一页（点标签就跳到这儿）。
    fn page_of_group(&self, group: usize) -> usize {
        self.ranges.get(group).map_or(0, |(first, _)| *first)
    }

    /// 第 `group` 个分类一共几页（分页细条画几格用它）。
    fn pages_of_group(&self, group: usize) -> usize {
        self.ranges
            .get(group)
            .map_or(1, |(first, last)| last - first + 1)
    }

    /// 第 `page` 页上那几个字符（不够一页就短）。
    fn page_items(&self, page: usize) -> &[String] {
        self.pages.get(page).map_or(&[][..], Vec::as_slice)
    }

    /// 一共有几页。
    fn page_count(&self) -> usize {
        self.pages.len()
    }

    /// 把「最近用过的」摆到最前面当一类（一条都没有时不摆）。
    ///
    /// emoji 与颜文字**共用一份**最近记录（搜狗那个「最近」也是不分类型的）——
    /// 刚用过的那个排第一，下次进来一眼就能点到。
    fn set_recent(&mut self, recent: &[String]) {
        let has = self.names.first().is_some_and(|name| name == RECENT_LABEL);
        if recent.is_empty() {
            // 空的：颜文字留着那一类（按钮的意头不变），表情不摆（少一格，别的分类宽一点）
            if has && !self.sticky_recent {
                self.names.remove(0);
                self.items.remove(0);
            } else if has {
                self.items[0] = Vec::new();
            }
        } else if has {
            self.items[0] = recent.to_vec();
        } else {
            self.names.insert(0, RECENT_LABEL.to_owned());
            self.items.insert(0, recent.to_vec());
        }
        // 页是按内容切的，动过就得重排（「最近」从无到有会多出一整个分类的页）
        self.rebuild();
    }

    /// 把现有分类**并成一个**（颜文字用）。
    ///
    /// 颜文字那页没有分类标签行（22 个中文分类排成小标签根本认不出来，用户说了删掉），
    /// 并成一条长表之后翻页就是从头滑到尾。名字留一个占位的「全部」——面板上不画它，
    /// 只是让「最近」仍然是第 0 类、别的还是第 1 类。
    fn merge_groups(&mut self) {
        let all: Vec<String> = self.items.drain(..).flatten().collect();
        self.names = vec!["全部".to_owned()];
        self.items = vec![all];
        self.rebuild();
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
        // 滤掉过整个分类，页要重排
        self.rebuild();
    }
}

/// 一个分类名在标签行上画什么。
///
/// 表情表里那九个分类各有图标（照 fcitx5-android 的选型，Google 的 Material Symbols）；
/// 颜文字那 22 个中文分类没有——一行挤 22 格，图标小到认不出，还是画两个字的名字。
fn group_label(name: &str) -> GroupLabel {
    let icon = match name {
        RECENT_LABEL => GroupIcon::Recent,
        "Smileys & Emotion" => GroupIcon::Smile,
        "People & Body" => GroupIcon::People,
        "Animals & Nature" => GroupIcon::Animals,
        "Food & Drink" => GroupIcon::Food,
        "Travel & Places" => GroupIcon::Travel,
        "Activities" => GroupIcon::Activities,
        "Objects" => GroupIcon::Objects,
        "Symbols" => GroupIcon::Symbols,
        "Flags" => GroupIcon::Flags,
        _ => return GroupLabel::Text(name.to_owned()),
    };
    GroupLabel::Icon(icon)
}

/// 表情面板这一刻该画什么：前后各一页 + 当前页、上面那条标签、分页细条。
///
/// **是个自由函数，不是会话的方法**：它借的是面板里的字符（`EmojiView` 存的是切片），
/// 而调用处紧接着要可变借键盘——走 `&self` 的方法就把整台会话借住了，借字段才拆得开。
fn emoji_view(panel: &EmojiPanel, scroll: f32, width: f32, density: f32) -> EmojiView<'_> {
    let width = width.max(1.0);
    let scale = density.max(1.0);
    let last = panel.page_count().saturating_sub(1);
    let page = ((scroll / width).floor().max(0.0) as usize).min(last);
    // 前后各一页：跟手时相邻那页要跟着露出来。两头没有的那一侧给空切片，
    // 渲染器照样按「±1 个整宽」摆，摆到视口外面的自然看不见。
    let before = if page > 0 {
        panel.page_items(page - 1)
    } else {
        &[]
    };
    let after = if page < last {
        panel.page_items(page + 1)
    } else {
        &[]
    };
    let group = panel.group_of_page(page);
    // 分页细条：这一类一共几页、已经翻过几页。**带小数**——跟手时细条要跟得住手指，
    // 一整格一整格跳的话看着像卡住了。
    let offset = (scroll - page as f32 * width) / width;
    let passed = (page - panel.page_of_group(group)) as f32 + offset;
    EmojiView {
        pages: [before, panel.page_items(page), after],
        shift: (scroll - page as f32 * width) / scale,
        labels: &panel.labels,
        group,
        pager: Some((passed, panel.pages_of_group(group))),
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

    /// 这份会话看的配置（`filesDir/config.toml`）：路径、上次读到的原文、当前生效的那一份。
    /// 换了新的一份由 [`Session::poll_config`] 推给引擎。
    config: ConfigState,

    /// 随包资源目录（壳从 APK 里解出来的：emoji、词表、模型、释义表、领域词库）。
    /// 换学习语言、重读词库都要回这儿找文件，所以留一份。
    bundle: Option<PathBuf>,

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

    /// 英文模式给不给候选（补全与拼错纠正）。
    ///
    /// **两个条件都成立才行**：随包的英文词表挂上了（[`Self::english_words`]），
    /// 且配置里 `[general] english_candidates` 开着（缺省开）。没词表时引擎给不出候选，
    /// 硬走组句只会把敲的字母攒在缓冲区里出不去，那时英文模式退回直输（字母直接打给应用）。
    english_candidates: bool,

    /// 随包的英文词表在不在。配置里那个开关是「允不允许」，这个才是「给不给得出来」——
    /// 词表没随包时，配置开着也没用。
    english_words: bool,

    /// 候选条画不画译文。**随包的释义表挂上了才是 `true`**，整场不变。
    ///
    /// 它不只是「画不画」：候选条的高度按它加一行（[`Self::bar_height`]），
    /// 而高度一变上面的应用内容就被顶——所以**必须是会话级的**，
    /// 不能看「这一屏有没有译文」临时决定（滚一格就跳一下）。
    /// 与「那行画不画」的区别见 [`Self::footer`]。
    annotations: bool,

    /// 云联想给的**整句补全**，画在候选条最下面那行右边。没结果时是 `None`。
    sentence: Option<String>,

    /// 云联想给的**词**，跟着本地候选一起排（见 [`CandidateLayout`]）。
    cloud_words: Vec<Candidate>,

    /// 壳报上来的光标前后文本。云联想拿它当上下文——联想准不准全看这个。
    surrounding: Option<SurroundingText>,

    /// 有请求在飞吗。壳照它决定要不要跑轮询心跳（见 [`Self::poll_prediction`]）。
    predicting: bool,

    /// 这一轮联想是什么时候发出去的。等太久了就别等了——`predict` 那边有超时，
    /// 但壳这边的「还要不要接着问」也得有个头，不然心跳停不下来。
    predicted_at: Option<Instant>,

    /// 键盘现在在哪一页。切页只换布局，键盘本身不高不矮。
    panel: Panel,

    /// 拼音行。没在组句时为 `None`。
    preedit: Option<Preedit>,

    /// 引擎给的本地候选，**整份列表**。云端词到了不算在它里面。
    ///
    /// 与 [`Self::candidates`] 分开是因为云端词会**反复重排**（每来一批结果就合一次），
    /// 而每次都得从「纯本地」那份重新算——不然上一轮的云端词会被当成本地的，越合越多。
    local_candidates: Vec<Candidate>,

    /// 本地候选 + 云端词，按 [`CandidateLayout`] 排好的样子。**真正画出去的**是它。
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

    /// 表情页**横滑的位移**（点，正数 = 内容往左走 = 在看后一页）。
    ///
    /// **只有零头那部分**——整页由 [`EmojiPanel::page`] 换掉（与剪贴板那份列表同一个分工：
    /// 整格的会话切，不足一格的渲染器让开）。松手时按它算「翻不翻、翻到哪一页」，
    /// 再起一段吸附动画滑过去（[`Slide`]）。
    emoji_page_scroll: f32,

    /// 刚才**真的横滑过表情格子**的那根手指（与 [`Self::scrolled`] 同一个用途）。
    emoji_page_scrolled: Option<i32>,

    /// 松手之后那一段收尾（从当前位置滑到整页）。
    emoji_page_slide: Option<Slide>,

    /// 剪贴板列表甩出去之后的那一段滑行（与候选条那条带子各走各的）。
    clipboard_fling: Option<Fling>,

    /// 剪贴板那把**锁**：锁上之后点一条粘完**不回字母页**，可以连着粘几条。
    ///
    /// 只在剪贴板页有效——切到别的页就解开（它的用途就是「这一次连着粘」，
    /// 不切走一直锁着的话，下次进来粘一条发现没回字母页会莫名其妙）。
    clipboard_locked: bool,

    /// 「清空」的第一下已经点过了、正等第二下确认（**5 秒内**有效）。
    ///
    /// 手滑一下就清光所有历史太狠，所以做成点两下：第一下把键帽改成「确认清空」、
    /// 第二下才真清（见 [`Self::request_clear`]）。过期**不靠定时器**——
    /// 那只有在要重画的时候才有意义，趁每次触摸顺手看一眼（[`Self::expire_clear`]）。
    clear_armed: Option<Instant>,

    /// **刚被「清空」清掉的那一条**（系统剪贴板里那条）。
    ///
    /// 键盘每次弹出来壳都会把系统剪贴板当前内容报一遍，不挡住的话刚清完又冒出来一条。
    /// 等报上来的内容跟它不一样了（用户复制了别的东西）就清掉这个标记。
    cleared_clipboard: Option<String>,

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

    /// 攒着「用户点了设置页」。壳用 [`Self::take_settings`] 取走。
    ///
    /// 跟 `pending_commands` 一个路数：会话碰不到安卓的窗口系统，
    /// 只把这件事记一笔，由壳去 `startActivity`。
    pending_settings: bool,

    /// 此刻按着的、**起手落在候选条上**的手指们，按根记。
    ///
    /// 键盘那半边的手指记在 [`Keyboard`] 自己手里，两边各记各的：一根手指属于谁，
    /// 由按下时落在哪半边决定，之后一直归它。这样抬起时不会因为手指划到了别处而丢掉这一下。
    pressed: Vec<BarPress>,

    /// 展开选词的面板开着没有（长按候选条弹出来，见 [`Self::open_expanded`]）。
    ///
    /// 开着时**键盘那张位图换成面板**（[`Self::keyboard_surface`] 在那儿分流）：
    /// 面板要盖住键盘、高度也照键盘来，所以壳那边一行都不用改——还是
    /// 「上面一条候选条、下面一张位图」，触摸也照样按 y 分派。
    expanded: bool,

    /// 面板被拉上去多少（点，纵向）。0 是第一行贴着面板顶边。
    ///
    /// 与剪贴板列表、表情格子**同一套做法**：跟手滚，随手停在哪儿都行。
    expanded_scroll: f32,

    /// 面板甩出去之后的那一段滑行（与别的几套各走各的）。
    expanded_fling: Option<Fling>,

    /// 刚才**真的滚过面板**的那根手指（与 [`Self::scrolled`] 同一个用途）。
    expanded_scrolled: Option<i32>,

    /// 画好的面板（位图 + 命中）。收起时是 `None`。
    ///
    /// 滚动当中**不丢掉旧的**：滚动上限要从这一帧的网格上问（格子是折行铺的，多高只有
    /// 铺完才知道），丢了它下一拍就滚不动了。要重画由 [`Self::expanded_dirty`] 说了算。
    expanded_view: Option<RenderedPanel>,

    /// 面板要重画。滚动一格、候选换了都置它，画完清掉。
    expanded_dirty: bool,

    /// 此刻按在面板上的那根手指。
    expanded_press: Option<PanelPress>,
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

/// 一根按在展开面板上的手指。
///
/// 面板**一次只认一根**：它是「一屏看全部」，没有多指同时点的道理，滚动本来就是单指的事。
/// 后来的一根会顶掉前一根（与候选条那批手指不一样，那边一根一根都记着）。
#[derive(Debug, Clone, Copy)]
struct PanelPress {
    /// 安卓给的 pointer id。不是它的事件一律不理。
    pointer: i32,

    /// 按下时的坐标（面板局部像素）。抬起时靠它判「挪没挪窝」。
    at: (f32, f32),

    /// 上一次报上来的纵坐标。竖滚要的是**位移增量**，与候选条那边同理。
    last: f32,

    /// 手指已经滑开了，这一下不再算「点击」——拖过就只是滚了一下，抬起不上屏。
    sliding: bool,
}

impl Session {
    /// 打开词库、建好引擎与渲染器。
    ///
    /// `locale` 决定中日同形字取哪家字形（`zh-CN` / `ja`）。`bundle` 是壳从 APK 里解出来的
    /// 随包资源目录，里面有 emoji 字体、emoji 表（见 `assets/emoji/README.md`）与英文词表，
    /// **有哪张用哪张**；`None` 表示都没有（用系统的 emoji 字体、不出 emoji 候选、英文模式退回直输）。
    pub fn open(
        dictionary_path: &Path,
        locale: &str,
        bundle: Option<&Path>,
        data_dir: Option<&Path>,
    ) -> Result<Self, SessionError> {
        // 配置先读：后面好几样（学习语言、英文候选、领域词库）都按它定。
        // 读的同时会把带注释的模板写出来（文件在就不动它），用户从此有份能手改的配置。
        let mut config = ConfigState::load(data_dir);
        let language = configured_language(&config.config().general);
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
        // 配置里那几样当场就能设的（模糊音、繁体、双拼、全角标点…）**建会话时就得推过去**：
        // 只留给 `poll_config` 的话，启动读到的那份永远补不上——文件没变它就返回 0，不会 apply。
        push_to_engine(&mut engine, config.config());
        // 云联想：配置开着、密钥也拿得到才接得上；接不上只用本地候选（失败只记日志）
        let predict = config.config().predict.clone();
        attach_cloud(&mut engine, &predict);
        config.set_predict(predict);
        if let Some(table) = bundle.and_then(load_emoji_tables) {
            tracing::info!(words = table.len(), "emoji 表已加载");
            engine = engine.with_emoji(table);
        }
        // 随包资源里那几样**可选**的数据：在一处读完，**每一样读不出来都只记日志**——
        // 与学习数据同一个取舍：少一样数据顶多是功能缺一块，输入法起不来是另一回事。
        let mut english_words = false;
        let mut annotations = false;
        if let Some(dir) = bundle {
            // 英文词表：挂上它，英文模式才有补全与拼错纠正
            let path = dir.join(ENGLISH_FILE);
            match WordList::from_path(&path) {
                Ok(words) => {
                    tracing::info!(path = %path.display(), words = words.len(), "英文词表已加载");
                    engine = engine.with_english(words);
                    english_words = true;
                }
                Err(error) => {
                    tracing::warn!(path = %path.display(), %error, "英文词表读不了，英文模式退回直输");
                }
            }
            // 语言模型：整句转换靠它，没有就退化成一元词频
            let path = dir.join(LANGUAGE_MODEL_FILE);
            match BigramModel::from_path(&path) {
                Ok(model) => {
                    tracing::info!(
                        path = %path.display(),
                        words = model.word_count(),
                        bigrams = model.bigram_count(),
                        "语言模型已加载"
                    );
                    engine = engine.with_language_model(Box::new(model));
                }
                Err(error) => {
                    tracing::warn!(path = %path.display(), %error, "语言模型读不了，整句退化成一元词频");
                }
            }
            // 释义表：挂了它候选条才有译文可画。两本各管一头——
            // `translator` 是「中文候选 → 学习语言」，`english_translator` 是「英文候选 → 中文」。
            // **学习语言是 `off` 就两本都不挂**：那时候整个译文那行都不该出现
            // （`annotations` 是假的，候选条也不留那行的高度）。
            if let Some(language) = language {
                let learning = dir.join(format!("glossary-{}.qj", language.code()));
                match Glossary::from_path(language, &learning) {
                    Ok(glossary) => {
                        tracing::info!(
                            path = %learning.display(),
                            language = language.code(),
                            "释义表已加载"
                        );
                        engine = engine.with_translator(Box::new(glossary));
                        annotations = true;
                        config.set_language(Some(language));
                    }
                    Err(error) => {
                        tracing::warn!(path = %learning.display(), %error, "学习语言的释义表读不了，候选条不画译文");
                    }
                }
                let english = dir.join(GLOSSARY_ENGLISH_FILE);
                match Glossary::from_path(Language::Chinese, &english) {
                    Ok(glossary) => {
                        tracing::info!(path = %english.display(), "英→中释义表已加载");
                        engine = engine.with_english_translator(Box::new(glossary));
                    }
                    Err(error) => {
                        tracing::warn!(path = %english.display(), %error, "英→中释义表读不了，英文候选没有中文释义");
                    }
                }
            }
            // 领域词库（成语 / 医学 / 法律 / 地名 …）：一个子目录，**有几本挂几本**。
            // 开哪几本由 `[dictionaries] domains` 定——安卓缺省 **11 本全开**
            // （`DictionariesConfig::default` 按平台分支，2026-09-22 定的），设置页可以逐本关。
            let dicts = dir.join(DICTS_DIR);
            let dictionaries = config.config().dictionaries.clone();
            let loaded = extra_dictionaries::load(Some(&dicts), None, &dictionaries);
            tracing::info!(books = loaded.len(), "领域词库已加载");
            engine.set_extra_dictionaries(loaded);
            config.set_dictionaries(dictionaries);
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
            EmojiPanel::open(&dir.join(EMOJI_PANEL_FILE), EMOJI_SLOTS, false)
        });
        let mut kaomoji_panel = bundle.map_or_else(EmojiPanel::default, |dir| {
            EmojiPanel::open(&dir.join(KAOMOJI_PANEL_FILE), KAOMOJI_SLOTS, true)
        });
        // 颜文字不分类：22 个中文分类做成标签根本认不出，全并成一条长表（2026-09-23）
        kaomoji_panel.merge_groups();
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

        // 英文候选：随包词表在不在是前提（没表就没候选可给），配置那个开关是「允不允许」
        let english_candidates = english_words && config.config().general.english_candidates;

        Ok(Self {
            engine,
            config,
            bundle: bundle.map(Path::to_path_buf),
            renderer,
            width: 0.0,
            density: 1.0,
            bottom_inset: 0.0,
            dark: false,
            landscape: false,
            keyboard: Some(Keyboard::new()),
            shift: ShiftState::default(),
            mode: InputMode::default(),
            english_candidates,
            english_words,
            annotations,
            sentence: None,
            cloud_words: Vec::new(),
            surrounding: None,
            predicting: false,
            predicted_at: None,
            panel: Panel::Letters,
            preedit: None,
            local_candidates: Vec::new(),
            candidates: Vec::new(),
            strip: BarStrip::default(),
            scroll: 0.0,
            fling: None,
            scrolled: None,
            emoji_recent,
            emoji: emoji_panel,
            kaomoji: kaomoji_panel,
            emoji_page_scroll: 0.0,
            emoji_page_scrolled: None,
            emoji_page_slide: None,
            clipboard: data_dir.map_or_else(Recent::default, |dir| {
                Recent::open(dir.join(CLIPBOARD_FILE), CLIPBOARD_LIMIT)
            }),
            clipboard_fling: None,
            clipboard_locked: false,
            clear_armed: None,
            cleared_clipboard: None,
            clipboard_scrolled: None,
            clipboard_scroll: 0.0,
            frame: Frame::default(),
            bar: None,
            bar_dirty: true,
            preedit_dirty: true,
            pending_commit: None,
            pending_commands: Vec::new(),
            pending_settings: false,
            pressed: Vec::new(),
            expanded: false,
            expanded_scroll: 0.0,
            expanded_fling: None,
            expanded_scrolled: None,
            expanded_view: None,
            expanded_dirty: false,
            expanded_press: None,
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
        // 键盘又弹出来了：展开面板收起来（用户要打字了，拼音与候选都留着）
        self.close_expanded();
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
        } else {
            // 锁只在剪贴板页里算数：切到别的页就解开（它的用途是「这一次连着粘」）
            self.clipboard_locked = false;
        }
        // 表情页：把跟手的零头和没跑完的动画清掉（上次停在哪一页还在哪一页，
        // 与剪贴板那边不一样——那边每次进来都回最新的几条，这边保留看的进度）
        if matches!(panel, Panel::Emoji | Panel::Kaomoji) {
            self.emoji_page_slide = None;
            self.emoji_page_scrolled = None;
            // 颜文字那页的「最近」是**常驻**的（右上角那个按钮得一直在），所以第 0 类
            // 总是它——但**进来该看「全部」**，不然头一次进来（还没用过什么）看到的
            // 就是一片空白。表情那页的「最近」空了就干脆不摆，从头看起就行。
            let start = if panel == Panel::Kaomoji {
                self.kaomoji.page_of_group(1)
            } else {
                0
            };
            self.emoji_page_scroll = start as f32 * self.emoji_page_width();
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
        Renderer::bar_height(&self.theme(), self.composing(), self.bottom_line())
    }

    /// 候选条**最下面那行**要不要留：挂了释义表、或者云联想开着。
    ///
    /// 那一行左边画译文、右边画云联想给的整句补全，两样都「可能没有」——
    /// 但高度**只看这两个会话级开关**，不看这一屏有没有东西可画
    /// （理由见 [`Self::annotations`]）。
    ///
    /// 名字不叫 `footer`：那个词在这份代码里已经是「第几页」（[`Frame::footer`]）。
    fn bottom_line(&self) -> bool {
        self.annotations || self.engine.prediction_enabled()
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
            // 先算好：闭包里借的是 `self.renderer`，里面再问 `self` 会撞借用
            let bottom_line = self.bottom_line();
            let rendered = self.renderer.as_mut().and_then(|renderer| {
                renderer
                    // 最后那个参数必须与 [`Self::bar_height`] **同一个来源**
                    // （`bottom_line()` = 有释义表 **或** 云联想开着）。
                    // 从前这儿只传了 `self.annotations`：开着云联想、没挂释义表时，
                    // 算高度按「有那一行」、画出来却没有——位图比壳以为的矮一整行，
                    // 于是**所有按键整体上偏**，按 `n` 出 `j`（用户 2026-09-23 报的）。
                    .render_bar(
                        &self.frame,
                        self.width,
                        &theme,
                        self.density,
                        scroll,
                        bottom_line,
                    )
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
        let emoji = emoji_view(panel, self.emoji_page_scroll, self.viewport(), self.density);
        match self.keyboard.as_mut() {
            Some(keyboard) => keyboard.popup_surface(
                self.renderer.as_mut(),
                shift,
                mode,
                ClipboardView {
                    entries: clipboard,
                    offset,
                    clear_armed: self.clear_armed.is_some(),
                    locked: self.clipboard_locked,
                },
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
    ///
    /// **面板开着时这里是面板**（[`Self::expanded_surface`]）：两者占同一块地方、同高同宽，
    /// 所以壳那边不必知道有两个东西——还是「上面一条候选条、下面一张位图」。
    pub fn keyboard_surface(&mut self) -> Vec<u8> {
        if self.expanded {
            return self.expanded_surface();
        }
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
        let emoji = emoji_view(panel, self.emoji_page_scroll, self.viewport(), self.density);
        match self.keyboard.as_mut() {
            Some(keyboard) => keyboard.surface(
                self.renderer.as_mut(),
                shift,
                mode,
                ClipboardView {
                    entries: clipboard,
                    offset,
                    clear_armed: self.clear_armed.is_some(),
                    locked: self.clipboard_locked,
                },
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

    /// 取走「用户点了设置页」（并清掉）。壳据此把键盘收起来、`startActivity`。
    ///
    /// 开 Activity 是壳的事——会话碰不到安卓的窗口系统，跟 [`Self::take_commit`]
    /// 是同一个路数：这边只记一笔，那边照着做。
    pub fn take_settings(&mut self) -> bool {
        std::mem::take(&mut self.pending_settings)
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
        // 「确认清空」过期了就复位（不用定时器，见 [`Self::expire_clear`]）
        if self.expire_clear() {
            self.mark_keyboard_dirty();
        }
        if matches!(action, MotionAction::Down | MotionAction::PointerDown) {
            // 手指一落下就把滑行停住：滑到一半想抓回来是「摸住就停」那个手感，
            // 不这么做的话按下去的那一下会和正在跑的惯性互相抢
            self.fling = None;
            self.scrolled = None;
            self.clipboard_fling = None;
            self.clipboard_scrolled = None;
            self.expanded_fling = None;
            self.expanded_scrolled = None;
            self.emoji_page_scrolled = None;
        }
        let bar_pixels = self.bar_pixels();
        if self.expanded && y >= bar_pixels {
            // 展开面板开着：键盘那块地方画的是面板，落在那里的触摸也就归它
            self.touch_expanded(action, pointer, x, y - bar_pixels);
        } else {
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
                    self.clipboard_scrolled = Some(pointer);
                    self.scroll_clipboard(delta);
                }
                Some(Fired::EmojiPageScroll(delta)) => {
                    // 记下是这根手指在横滑——抬手时靠它判「翻不翻页、翻到哪一页」
                    self.emoji_page_scrolled = Some(pointer);
                    // 手指按下来就把还在跑的那段吸附动画停掉（「摸住就停」那个手感）
                    self.emoji_page_slide = None;
                    self.scroll_emoji_page(delta);
                }
                None => {}
            }
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
        // 手指按在**候选条**上够久了：展开选词（K13）。
        //
        // 入口选长按是因为候选条右端已经有 `×` 和 `‹ ›` 三块，再加一个太挤；
        // 而长按在候选条上本来就没用上（K8 那个「长按删候选」2026-09-20 摘掉了）。
        if self.pressed.iter().any(|held| held.pointer == pointer) {
            self.open_expanded();
            return self.mask();
        }
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
                // 面板开着的时候，候选条这一条就是「面板外面」：点它收起来，那一下不上屏
                if self.expanded {
                    self.close_expanded();
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
            // 不认识的事件：什么都不做（见 [`MotionAction::Ignore`]）
            MotionAction::Ignore => {}
        }
    }

    /// 展开选词：把候选铺成一块**盖住键盘**的多行面板。
    ///
    /// 入口是长按候选条（[`Self::repeat`]）；出口有三个——点一个上屏、点候选条、
    /// 按返回（壳报进来，走 [`Self::dismiss`]）。
    ///
    /// 一个候选都没有时不开：空面板既没得选又占掉整块键盘，不如什么都不发生。
    fn open_expanded(&mut self) {
        if self.expanded || self.candidates.is_empty() {
            return;
        }
        self.expanded = true;
        self.expanded_scroll = 0.0;
        self.expanded_fling = None;
        self.expanded_scrolled = None;
        self.expanded_dirty = true;
        // **这一下从「按着候选条」变成「开了面板」**：手指还按着，但抬手时不该再算成
        // 点了一下候选条（那会上屏），所以把它从候选条那批手指里摘掉
        self.pressed.clear();
    }

    /// 收起面板。收起之后键盘要重画——那块地方换回键盘了。
    fn close_expanded(&mut self) {
        if !self.expanded {
            return;
        }
        self.expanded = false;
        self.expanded_scroll = 0.0;
        self.expanded_fling = None;
        self.expanded_scrolled = None;
        self.expanded_view = None;
        self.expanded_dirty = false;
        self.expanded_press = None;
        self.mark_keyboard_dirty();
    }

    /// 返回键按下了：把开着的那层收掉。
    ///
    /// 返回 [`flags`] 的掩码，**0 表示这一下不归我们管**（壳照常把返回交给应用）。
    /// 面板收起来、页切回字母页都一定带着 [`flags::KEYBOARD`] 那一位，
    /// 所以「非 0 = 我处理了」这条判断成立。
    pub fn dismiss(&mut self) -> i32 {
        if self.expanded {
            self.close_expanded();
        } else if self.panel != Panel::Letters {
            // 工具页 / 表情页 / 剪贴板页开着：返回先回字母页，跟主流输入法一致
            self.set_panel(Panel::Letters);
        } else {
            return 0;
        }
        self.mask()
    }

    /// 展开面板的位图（8 字节头 + 预乘 RGBA）。**它占的是键盘那块地方**。
    ///
    /// 没画过或者要重画时现画一张。每格摆在哪儿由渲染器量（与候选条同一套量宽），
    /// 这里只管把「整份候选、占多大地方、滚到哪儿」三样喂进去。
    fn expanded_surface(&mut self) -> Vec<u8> {
        if self.expanded_dirty || self.expanded_view.is_none() {
            let rows = self.rows_of(0..CANDIDATE_LIMIT);
            let theme = self.theme();
            let area = PanelArea {
                width: self.width,
                // 与键盘位图同高：键盘的高度是「键排到哪儿」，位图下面还给系统手势条
                // 留了一段（`render_keyboard` 加上去的），面板得跟着，不然视图高度会变
                height: self.keyboard_height() + self.bottom_inset,
                bottom_inset: self.bottom_inset,
            };
            let rendered = self.renderer.as_mut().and_then(|renderer| {
                renderer
                    .render_candidates_panel(
                        &rows,
                        area,
                        &theme,
                        self.density,
                        self.expanded_scroll,
                    )
                    .ok()
            });
            let Some(rendered) = rendered else {
                return Vec::new();
            };
            self.expanded_view = Some(rendered);
            self.expanded_dirty = false;
        }
        self.expanded_view
            .as_ref()
            .map_or_else(Vec::new, |view| surface::encode(&view.rendered.pixmap))
    }

    /// 展开面板那半边的触摸。`x` / `y` 是**面板局部**的像素（壳已减掉候选条高度）。
    ///
    /// 与候选条一个路数，只是方向换了：那边横着滚，这边**竖着滚**。
    /// 点一下上屏——面板上除了候选格没有别的可点的东西，命中哪格就上屏哪个。
    fn touch_expanded(&mut self, action: MotionAction, pointer: i32, x: f32, y: f32) {
        match action {
            MotionAction::Down | MotionAction::PointerDown => {
                self.expanded_press = Some(PanelPress {
                    pointer,
                    at: (x, y),
                    last: y,
                    sliding: false,
                });
            }
            MotionAction::Move => {
                let slop = self.touch_slop();
                let Some(press) = self
                    .expanded_press
                    .as_mut()
                    .filter(|press| press.pointer == pointer)
                else {
                    return;
                };
                if !within_slop(press.at, slop, x, y) {
                    press.sliding = true;
                }
                // 手指往上移 = 内容往上走 = 看后面的候选。**只认这一下的位移增量**，所以是跟手的
                let step = press.last - y;
                press.last = y;
                if press.sliding && step != 0.0 {
                    self.scroll_expanded(step);
                    // 记下是**这根手指**在滚：只有它抬起时才谈得上甩
                    self.expanded_scrolled = Some(pointer);
                }
            }
            MotionAction::Up | MotionAction::PointerUp => {
                let Some(press) = self.expanded_press.filter(|press| press.pointer == pointer)
                else {
                    return;
                };
                self.expanded_press = None;
                // 拖过就只是滚了一下，**不上屏**——与候选条同一条规矩
                // （要「滚到哪儿就选哪个」的话，一路翻看过去就没法全身而退了）
                if press.sliding {
                    return;
                }
                let scroll = self.expanded_scroll;
                let index = self
                    .expanded_view
                    .as_ref()
                    .and_then(|view| view.hit(x, y, scroll));
                if let Some(index) = index {
                    self.commit_candidate(index);
                }
            }
            MotionAction::Cancel => self.expanded_press = None,
            MotionAction::Ignore => {}
        }
    }

    /// 面板竖着滚 `step` 像素（正数 = 内容往上走，看后面的候选）。
    ///
    /// 两端夹住：滚到头再拖也拖不出空白。上限从**这一帧的网格**上问——
    /// 面板的格子是折行铺的，多高只有铺完才知道。
    fn scroll_expanded(&mut self, step: f32) {
        let max = self.expanded_max_scroll();
        let wanted = (self.expanded_scroll + step).clamp(0.0, max);
        if wanted == self.expanded_scroll {
            return;
        }
        self.expanded_scroll = wanted;
        self.expanded_dirty = true;
    }

    /// 面板此刻最远能滚到哪儿（像素）。还没画过时是 0——那时也滚不动。
    fn expanded_max_scroll(&self) -> f32 {
        self.expanded_view
            .as_ref()
            .map_or(0.0, |view| view.max_scroll(self.panel_pixels()))
    }

    /// 面板在输入视图里占的高度（像素）——就是键盘那么高（它盖住键盘）。
    fn panel_pixels(&self) -> f32 {
        self.keyboard_height() * self.density
    }

    /// 上屏**整份候选里**第 `index` 个。
    ///
    /// 两条路都走这儿：候选条的命中给的是页内下标（调用处先换算成整份的），
    /// 展开面板给的本来就是整份的下标。
    fn commit_candidate(&mut self, index: usize) {
        if let Some(candidate) = self.candidates.get(index).cloned() {
            let text = self.engine.commit(&candidate);
            self.commit_text(text);
        }
    }

    /// 这一轮下来哪些面要重取、有没有话要交给应用。
    fn mask(&self) -> i32 {
        let mut mask = 0;
        // 面板开着时**键盘那块地方画的是面板**，所以「要不要重取位图」看的是面板脏不脏；
        // 没开面板时才轮到键盘。**这次不报下次就没人报了**——`expanded_surface` 一画完
        // 就把脏标记清掉，掩码不再带这一位，壳那边也就跟着停了。
        if self.expanded {
            if self.expanded_dirty || self.expanded_view.is_none() {
                mask |= flags::KEYBOARD;
            }
        } else if self.keyboard.as_ref().is_some_and(Keyboard::dirty) {
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
        if self.fling.is_some()
            || self.clipboard_fling.is_some()
            || self.expanded_fling.is_some()
            || self.emoji_page_slide.is_some()
        {
            mask |= flags::FLING;
        }
        if self.pending_settings {
            mask |= flags::SETTINGS;
        }
        if self.predicting {
            mask |= flags::PREDICTING;
        }
        mask
    }

    /// 候选条在整块输入视图里占的高度（像素）。键盘接在它下面。
    fn bar_pixels(&self) -> f32 {
        // **必须与渲染器算位图高度时同一个口径**（那边是 `(高 × 密度).round()`，见
        // `render_bar`）：这个数是壳拿来把触摸 y 换算到键盘的偏移，差一像素，
        // 所有按键就整体偏一像素——边缘上按就串到隔壁键（2026-09-23 的测试量出来的）。
        (self.bar_height() * self.density).round()
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
            Act::Push(c) => self.type_letter(c),
            Act::CommitCandidate(index) => {
                // 命中矩形里的下标是**画出来那一批**里的（从最左边看得见的那个数起）
                self.commit_candidate(self.visible().start + index);
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
            Act::CommitTranslation(sense) => {
                // 点候选底下那行小字：上屏**译文**而不是候选词；那行摆着两条时点哪条上屏哪条。
                // 学习记账、拼音消耗、个人词表都由引擎按「选了那个候选」办（`commit_translation`），
                // 这里只管把返回的那串文本交出去。那一条不存在就什么都不做——
                // 不过命中区只在那条真画出来时才有，正常点不到这种。
                let translated = self
                    .highlighted_candidate()
                    .and_then(|candidate| self.engine.commit_translation(&candidate, sense));
                if let Some(text) = translated {
                    self.commit_text(text);
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
                // 没词表就别让引擎走英文那条路——给不出候选，字母只会攒在缓冲区里出不去
                self.engine
                    .set_english_mode(self.mode == InputMode::English && self.english_candidates);
                self.mark_keyboard_dirty();
                self.recompose();
            }
            Act::Punctuate(c) => self.punctuate(c),
            Act::ToggleTools => self.toggle_tools(),
            // 会话开不了 Activity，只记一笔；掩码里带上 SETTINGS，壳那边去开
            Act::OpenSettings => self.pending_settings = true,
            Act::Nothing => {}
            Act::AcceptPrediction => self.accept_prediction(),
            Act::PasteClipboard(index) => self.paste_clipboard(index),
            Act::DeleteClipboard(index) => self.delete_clipboard(index),
            Act::ClearClipboard => self.request_clear(),
            Act::ToggleClipboardLock => self.toggle_clipboard_lock(),
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
    /// 英文模式分两条：
    /// - **词表在**（[`Session::english_candidates`]）：字母进组句缓冲区，出补全与拼错纠正。
    ///   大小写在这儿定好再交给引擎——英文里大小写是有意义的，而引擎只拿到最终字符，
    ///   候选的大小写由它照着这段输入回推（`Comp` → `Company`、`COMP` → `COMPANY`）。
    /// - **词表不在**：**直输**，字母不进缓冲区、直接打给应用。这正是桌面壳关掉英文候选
    ///   （`[general] english_candidates = false`）时走的那条路。
    fn type_letter(&mut self, c: char) {
        if self.english() {
            let c = if self.shift.is_upper() {
                c.to_ascii_uppercase()
            } else {
                c.to_ascii_lowercase()
            };
            if self.english_candidates {
                self.engine.push(c);
                self.recompose();
            } else {
                self.engine.note_passthrough(c);
                self.commit_text(c.to_string());
            }
            return;
        }
        self.engine.push(c.to_ascii_lowercase());
        self.recompose();
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

    /// 本地候选 + 已经拿到的云端词，按 [`CandidateLayout`] 排好。
    ///
    /// 云端词占**第一页末尾那几格**（配置 `[predict] slots`），前面的本地候选不动——
    /// 与桌面两壳同一套排法。安卓候选条不分页，但「插在第 7、8 个位置」这个语义照样成立。
    fn merge_cloud(&self) -> Vec<Candidate> {
        if self.cloud_words.is_empty() {
            return self.local_candidates.clone();
        }
        let config = self.config.config();
        let mut layout = CandidateLayout::new(
            self.local_candidates.clone(),
            config.general.page_size(),
            config.predict.slots,
        );
        layout.set_cloud(self.cloud_words.clone());
        layout
            .cells()
            .into_iter()
            .filter_map(|cell| match cell {
                Cell::Local(candidate) | Cell::Cloud(candidate) => Some(candidate.clone()),
                Cell::Empty => None,
            })
            .collect()
    }

    /// 缓冲变了就问问云端（接着了才问）。
    ///
    /// **每次都发**：防抖在 `qingjian-predict` 的 worker 里做（`debounce_ms`，缺省 300ms），
    /// 它只把最后一个真发出去——壳不用自己算「停键多久了」。
    fn request_prediction(&mut self) {
        if !self.engine.prediction_enabled() {
            return;
        }
        let surrounding = self.surrounding.clone();
        match self
            .engine
            .request_prediction(surrounding, &self.local_candidates)
        {
            Some(_) => {
                self.predicting = true;
                self.predicted_at = Some(Instant::now());
            }
            // 引擎自己判定不发（私密输入、拼音不像话、字母太少）：这边就别留着心跳了
            None => self.stop_predicting(),
        }
    }

    /// 看云联想有没有结果回来。**壳在掩码带 [`flags::PREDICTING`] 时按拍子问**。
    ///
    /// 拿到结果就更新整句与云端词、重排候选；还没到、也没等太久就返回 0（壳那边什么都不做）。
    /// 等超过 [`PREDICTION_WAIT`] 就收摊——**不预留、不画占位**，这一轮就是没有变化。
    pub fn poll_prediction(&mut self) -> i32 {
        if !self.predicting {
            return 0;
        }
        let Some(prediction) = self.engine.poll_prediction() else {
            if self
                .predicted_at
                .is_some_and(|at| at.elapsed() >= PREDICTION_WAIT)
            {
                tracing::debug!("云联想等太久了，这一轮不等了");
                self.stop_predicting();
                return 0;
            }
            // **还没回来也得留着这一位**：壳看它就是「接着问下一拍」。
            // 返回 0 的话壳以为不用再问了，结果永远收不到（2026-09-22 在模拟器上抓到过：
            // 日志是「轮询了，还没有 → 联想完成」，中间没有第二次轮询）。
            return flags::PREDICTING;
        };
        self.stop_predicting();
        self.sentence = prediction.sentence;
        self.cloud_words = prediction
            .words
            .into_iter()
            .map(CloudWord::into_candidate)
            .collect();
        // 云端词到了：重排一次候选。**不重新问云**——刚拿到的就是这一串的答案。
        // `refresh` 不能省：它才是标脏的那个（`relayout` 只重铺带子），
        // 少了它壳收到 0、候选条上什么都不会变（2026-09-22 在模拟器上抓到过）。
        self.candidates = self.merge_cloud();
        self.relayout();
        self.refresh();
        self.mask()
    }

    /// 收摊：不再等这一轮的结果。
    fn stop_predicting(&mut self) {
        self.predicting = false;
        self.predicted_at = None;
    }

    /// 接受云联想给的整句补全：上屏它，并把这一轮的联想收掉。
    ///
    /// 上屏的文本由引擎算（它还要记个人 n-gram、按拼音消耗缓冲区），壳只管交出去。
    /// 电脑上这一步是 Tab 键，安卓是点候选条最下面那行右边那段（[`BarHitId::Sentence`]）。
    fn accept_prediction(&mut self) {
        let Some(sentence) = self.sentence.take() else {
            return;
        };
        self.stop_predicting();
        let text = self.engine.accept_prediction(&sentence);
        // `commit_text` 会顺手重查一遍（缓冲区变了），整句与云端词跟着清掉
        self.commit_text(text);
    }

    /// 私密输入框（密码框）里：**不学、不记、不发云端**（引擎自己有一道闸）。
    ///
    /// 壳在 `onStartInputView` 里按 `EditorInfo.inputType` 判出来报给它。
    /// 进密码框时这一轮的联想一并作废——**已经发出去的那次也当没发生**。
    pub fn set_private(&mut self, private: bool) {
        self.engine.set_private(private);
        if !private {
            return;
        }
        self.stop_predicting();
        self.sentence = None;
        self.cloud_words.clear();
        // 重排一次：云端词从候选里撤掉（`request_prediction` 会因为私密而不再发）
        self.recompose();
    }

    /// 壳报上来的光标前后文本。**云联想拿它当上下文**——联得准不准全看这个。
    ///
    /// 壳在 `onStartInputView` 时问一次应用的输入框（`getTextBeforeCursor` / `AfterCursor`）。
    /// 太长不要紧，引擎会按 `[predict] lookback / lookahead` 自己裁。
    pub fn set_surrounding(&mut self, before: String, after: String) {
        self.surrounding = Some(SurroundingText { before, after });
    }

    /// 缓冲变了：重查候选，带子从头铺开、也从头上看起。
    fn recompose(&mut self) {
        self.scroll = 0.0;
        // 滑行是从「刚才滚到哪儿」接着走的，候选一换就没意义了
        self.fling = None;
        self.scrolled = None;
        self.candidates.clear();
        self.local_candidates.clear();
        self.preedit = None;
        // 没在组句了：整句补全与云端词都收掉——它们只对「正在打的这一串」有意义
        self.sentence = None;
        self.cloud_words.clear();
        self.stop_predicting();
        if !self.engine.composition().is_empty() {
            match self.engine.query() {
                Ok(mut query) => {
                    let segments = query.marked_segments();
                    self.preedit = Some(Preedit {
                        segments: segments.iter().map(preedit_segment).collect(),
                        cursor: query.marked_cursor(),
                    });
                    // 译文是**后补的**：先按词级排序出候选，再问释义表把译文贴上。
                    // 释义表是 mmap 的，这一步就是几次查表，不吃什么时间；
                    // 贴不上就留空（`docs/design/candidate-ui.md`：译文缺失时那行留空，不显示占位符）。
                    if self.annotations {
                        self.engine.annotate(&mut query.candidates);
                    }
                    self.local_candidates = query.candidates.items;
                    self.candidates = self.merge_cloud();
                    // 缓冲变了就再问一次云。**每次都发**——防抖在 `qingjian-predict` 的
                    // worker 里做（`debounce_ms`，缺省 300ms），它只把最后一个真发出去。
                    self.request_prediction();
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
        // 展开面板开着时：候选换了要重画（云联想的词回来也算），一个都不剩就收起来——
        // 空面板既没得选、还占着整块键盘（上屏之后就是这种情况）
        if self.expanded {
            if self.candidates.is_empty() {
                self.close_expanded();
            } else {
                self.expanded_dirty = true;
            }
        }
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

    /// 候选中 `range` 这一段转成要画的行。
    ///
    /// **转法只能有一份**：高亮、云端词的云朵、译文的挂法都在 [`row`] 里定，
    /// 三处（候选条看得见的那几格、整条带子、展开面板）各转各的迟早会不一致。
    /// 越界的段按实际长度夹住，不 panic。
    fn rows_of(&self, range: Range<usize>) -> Vec<Row> {
        let end = range.end.min(self.candidates.len());
        let start = range.start.min(end);
        self.candidates[start..end]
            .iter()
            .enumerate()
            .map(|(i, candidate)| row(start + i, candidate))
            .collect()
    }

    /// 画哪几个候选、页码是几。这一轮不画译文，所以不调 `engine.annotate()`。
    ///
    /// **只画看得见的那几格**（十来个，两边各带一个只露半边的），不是「铺一整批」——
    /// 带子是通的，滚到哪儿画哪儿。
    fn build_frame(&self) -> Frame {
        let visible = self.visible();
        let rows = self.rows_of(visible.clone());
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
            // 云联想给的整句补全（画在候选条最下面那行右边）；没结果时是 None
            sentence: self.sentence.clone(),
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
        let rows = self.rows_of(0..CANDIDATE_LIMIT);
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
        if self.emoji_page_scrolled == Some(pointer) {
            // 表情页横滑松手：按「手速 + 拖了多远」定翻不翻页，再起一段吸附动画
            // （照 ViewPager2 的手感，2026-09-23）。**速度在这儿是真用上了**——
            // 原来那一版整个丢掉，轻轻一甩不翻页，用户说手感不对就是这个。
            self.settle_emoji_page(velocity_x);
        } else if self.expanded_scrolled == Some(pointer) {
            // 面板竖着滚：手指往上甩（速度为负）= 内容往上走 = `scroll_expanded` 变大，
            // 与那边收的「手指挪了多少」同向，所以符号**不取反**；除以 1000 换成像素/毫秒
            self.expanded_fling = Fling::new(velocity_y / 1000.0);
        } else if self.clipboard_scrolled == Some(pointer) {
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
        // **刚清掉的那条别再记回来**：键盘每次弹出来，壳都会把系统剪贴板里当前那条
        // 报一遍（`onStartInputView` 那条路），不挡的话「清空」刚点完就又冒出来一条
        // （用户 2026-09-22 报的）。等系统剪贴板真变了（下面那行）自然就过去了。
        if self.cleared_clipboard.as_deref() == Some(text) {
            return self.mask();
        }
        self.cleared_clipboard = None;
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

    /// 表情页点了一个：**整条上屏**（不是像打字那样一个字符一个字符喂给引擎）。
    ///
    /// 上屏之后**留在这一页**——发 emoji 常常一次发好几个，弹回字母页反而要重新点进来。
    fn commit_emoji(&mut self, index: usize) {
        let page = self.emoji_page();
        let Some(text) = self.emoji_panel().page_items(page).get(index).cloned() else {
            return;
        };
        self.commit_text(text.clone());
        // 用过就记进「最近」，下次进来排在最前面（两页共用一份，所以两页都要更新）
        if self.emoji_recent.remember(&text) {
            // 记住**原来在哪一类的第几页**：新的一条会让「最近」那一类的页变多
            // （从无到有更是一下多出一整类），后面的页整体往后挪。
            // 只把位移夹回范围是不够的——页号没动、内容挪了，看到的就是隔壁那一页
            // （模拟器上 2026-09-23 抓到：点了面旗帜，画面跳去符号类）。
            let page = self.emoji_page();
            let (name, within) = {
                let panel = self.emoji_panel();
                let group = panel.group_of_page(page);
                (
                    panel.names.get(group).cloned(),
                    page - panel.page_of_group(group),
                )
            };
            let recent = self.emoji_recent.entries().to_vec();
            self.emoji.set_recent(&recent);
            self.kaomoji.set_recent(&recent);
            self.restore_emoji_page(name.as_deref(), within);
            self.mark_keyboard_dirty();
        }
    }

    /// 点了分类标签：跳到**那一类的第一页**。
    ///
    /// `index` 就是标签行上第几格——标签是**全部分类平铺**的，格号就是分类号
    /// （2026-09-23 起。原来一屏只摆五个，还得加上这一屏的起点换算）。
    fn pick_emoji_group(&mut self, index: usize) {
        // 颜文字页没有分类标签行，唯一那个按钮是**右上角的「最近」**，做成开关：
        // 点一下看最近用过的，再点一下回到全部（`sticky_recent` 保证 0 就是「最近」）。
        if matches!(self.panel, Panel::Kaomoji) {
            let target = if self.emoji_page_group() == 0 { 1 } else { 0 };
            self.set_emoji_group(target.max(index.min(1)));
            return;
        }
        self.set_emoji_group(index);
    }

    /// 跳到第 `target` 个分类的第一页（点标签走这儿）。
    fn set_emoji_group(&mut self, target: usize) {
        let page = {
            let panel = self.emoji_panel();
            if target >= panel.names.len() {
                return;
            }
            panel.page_of_group(target)
        };
        self.emoji_page_scroll = page as f32 * self.emoji_page_width();
        // 换了地方，没跑完的那段动画作废
        self.emoji_page_slide = None;
        self.mark_keyboard_dirty();
    }

    /// 一页多宽（像素）——就是一屏：整块键盘那么宽。
    fn emoji_page_width(&self) -> f32 {
        self.viewport()
    }

    /// 现在在看第几页（整份 `pages` 里的下标）。
    ///
    /// **从位移算**，不另外存一个「第几页」：跟手拖到一半时「现在算哪一页」本来就是个
    /// 中间态（`floor` 过去就到下一页了），存两份迟早不同步。
    fn emoji_page(&self) -> usize {
        let width = self.emoji_page_width();
        if width <= 0.0 {
            return 0;
        }
        let last = self.emoji_panel().page_count().saturating_sub(1);
        ((self.emoji_page_scroll / width).floor().max(0.0) as usize).min(last)
    }

    /// 当前这一页属于第几个分类（上面那条标签画哪一个高亮；颜文字那页则是
    /// 「右上角那个按钮现在是看最近还是看全部」）。
    fn emoji_page_group(&self) -> usize {
        self.emoji_panel().group_of_page(self.emoji_page())
    }

    /// 表情页还能往后翻多少（像素）：后面还有几页，一页一格。
    fn emoji_page_max_scroll(&self) -> f32 {
        let last = self.emoji_panel().page_count().saturating_sub(1);
        last as f32 * self.emoji_page_width()
    }

    /// 内容换过之后，按**原来那一类的名字 + 类内第几页**把位置重新落回去。
    ///
    /// 页号当然会变（新条目让「最近」那一类多一页、从无到有多一整类），但**分类的下标
    /// 也会漂**——「最近」永远插在最前面，它一出现，后面每个分类的下标就整体 +1。
    /// 所以不能拿旧下标去查新表（那样定位到的是隔壁那一类，模拟器上 2026-09-23 抓到：
    /// 点了面旗帜，上屏之后画面跳去了符号类），得**按名字**认。
    ///
    /// 名字对新表也对不上时退回第一页——总比停在一个说不清的页上强。
    fn restore_emoji_page(&mut self, name: Option<&str>, within: usize) {
        let target = {
            let panel = self.emoji_panel();
            name.and_then(|name| panel.names.iter().position(|each| each == name))
                .map(|group| panel.page_of_group(group) + within)
        };
        let Some(target) = target else {
            self.emoji_page_scroll = 0.0;
            return;
        };
        self.emoji_page_scroll = target as f32 * self.emoji_page_width();
        // 页数可能反而变少了，落位之后再夹一道
        self.clamp_emoji_page();
    }

    /// 把位移夹回「现在这几页」的范围里。
    ///
    /// **页数会变少**（「最近」清空过、滤过画不出来的字形），位移不跟着夹就会指到
    /// 不存在的页上——屏幕上是一片空，而且怎么滑都回不来。
    fn clamp_emoji_page(&mut self) {
        let max = self.emoji_page_max_scroll();
        self.emoji_page_scroll = self.emoji_page_scroll.clamp(0.0, max);
    }

    /// 表情页的格子跟着手指横着挪（`delta` 是这一拍挪了多少**像素**，往右为正）。
    ///
    /// 手指往右拖是把内容往右带、看的是更前面的一页，所以位移是**减**（正数 = 内容往左走）。
    /// 夹在 `[0, 还能翻多少]` 里：头一页往前、末一页往后都拖不动
    /// （ViewPager2 也是硬边界，不是橡皮筋回弹）。
    fn scroll_emoji_page(&mut self, delta: f32) {
        let max = self.emoji_page_max_scroll();
        let wanted = (self.emoji_page_scroll - delta).clamp(0.0, max);
        if wanted == self.emoji_page_scroll {
            return;
        }
        self.emoji_page_scroll = wanted;
        self.mark_keyboard_dirty();
    }

    /// 表情页横滑松手：**按手速与拖了多远定翻到哪一页**，再起一段吸附动画滑过去。
    ///
    /// 判定照 fcitx5-android 用的那个 `ViewPager2`（`PagerSnapHelper` + `SnapHelper::onFling`）：
    ///
    /// 1. **手速够就翻页**：朝手甩的方向翻一页，跟拖了多远无关。这条阈值**特别低**
    ///    （安卓 `ViewConfiguration` 的 `scaledMinimumFlingVelocity`，50 dp/s），
    ///    轻轻一拨就过——「轻甩一下也能翻页」全靠它。
    /// 2. **手速不够看拖了多远**：拖过**半页**翻一页，不到半页回原位。
    /// 3. 两头（头一页往前、末一页往后）到头就不动。
    ///
    /// **不是直接赋值跳过去**：从当前位置起一段减速动画（[`Slide`]）。
    /// 用户 2026-09-23 说的「硬跳」就是少了这一段。
    fn settle_emoji_page(&mut self, velocity_x: f32) {
        let width = self.emoji_page_width();
        if width <= 0.0 {
            return;
        }
        let page = self.emoji_page();
        let last = self.emoji_panel().page_count().saturating_sub(1);
        let offset = self.emoji_page_scroll - page as f32 * width;
        // 「现在离得最近的那一页」：下一页露过半屏就轮到它了——手速不够时吸附到它。
        let near = if offset > width / 2.0 {
            (page + 1).min(last)
        } else {
            page
        };
        // 手指往左甩（速度为负）= 看后面一页 = 内容往左走，所以符号取反；
        // 再除以 1000 换成「像素/毫秒」，与 [`Fling`] 那套同一个单位
        let speed = -velocity_x / 1000.0;
        let target = if speed.abs() >= MIN_PAGE_FLING * self.density {
            // 手速够：**只看方向**，拖了多远不算
            if speed > 0.0 {
                (near + 1).min(last)
            } else {
                near.saturating_sub(1)
            }
        } else {
            near
        };
        // 落定的位置就是「第 target 页正对着视口」，动画从当前位置滑过去。
        // 已经在目标上（没拖动、也没甩）时 `Slide::new` 给 `None`，不起动画。
        self.emoji_page_slide = Slide::new(self.emoji_page_scroll, target as f32 * width);
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
        // 插完收回字母页——**锁上了就不回**（锁就是给「连着粘几条」用的）
        if !self.clipboard_locked {
            self.set_panel(Panel::Letters);
        }
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
    /// 拨一下剪贴板那把锁（锁上 / 解开）。
    fn toggle_clipboard_lock(&mut self) {
        self.clipboard_locked = !self.clipboard_locked;
        self.mark_keyboard_dirty();
    }

    /// 点「清空」：**第一下只是预备，第二下才真清**（用户 2026-09-22 要的）。
    ///
    /// 手滑一下就清光所有历史太狠。第一下之后键帽改口说「确认清空」，再过 `CLEAR_CONFIRM_WINDOW`
    /// 之内点第二下才清；超时或者在这之间点了别处，就退回原样（下次还得点两下）。
    fn request_clear(&mut self) {
        // 顺手看一眼过没过期：过期了就当这一下是新一轮的第一下
        self.expire_clear();
        if self.clear_armed.is_some() {
            self.clear_armed = None;
            self.clear_clipboard();
            return;
        }
        self.clear_armed = Some(Instant::now());
        self.mark_keyboard_dirty();
    }

    /// 「确认清空」这类状态过期了没有；过期的顺手复位。返回要不要重画。
    ///
    /// **不用定时器**：它只在要重画的时候才有意义，而每次触摸本来就要经过这儿
    /// （见 [`Self::touch`]）。代价是「过期之后不碰键盘，键帽上那句提醒会一直留着」——
    /// 无所谓，反正一碰就复位了，而且那时候点下去也只会重新开始确认。
    fn expire_clear(&mut self) -> bool {
        let Some(at) = self.clear_armed else {
            return false;
        };
        if at.elapsed() < CLEAR_CONFIRM_WINDOW {
            return false;
        }
        self.clear_armed = None;
        true
    }

    /// 清空剪贴板历史（第二下确认之后才走到这儿）。
    fn clear_clipboard(&mut self) {
        // 记住**刚清掉的是哪一条**（历史里最新那条就是系统剪贴板里那条）——
        // 下次键盘弹出来时壳会把它再报一遍，`note_clipboard` 靠这个标记挡住。
        self.cleared_clipboard = self.clipboard.entries().first().cloned();
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
        // 剪贴板那份列表（竖着滚）。几套各记各的速度，但同一时刻只有一套会在跑——
        // 面板开着时没有候选条、也没有剪贴板页，反过来也一样
        if let Some(fling) = self.clipboard_fling.as_mut() {
            let step = fling.step(dt);
            let finished = fling.finished();
            let before = self.clipboard_scroll;
            self.scroll_clipboard(step);
            if finished || (step != 0.0 && self.clipboard_scroll == before) {
                self.clipboard_fling = None;
            }
        }
        // 展开面板（也竖着滚，与剪贴板同一套，只是走自己的位移）
        if let Some(fling) = self.expanded_fling.as_mut() {
            let step = fling.step(dt);
            let finished = fling.finished();
            let before = self.expanded_scroll;
            self.scroll_expanded(step);
            if finished || (step != 0.0 && self.expanded_scroll == before) {
                self.expanded_fling = None;
            }
        }
        // 表情页松手之后那一段吸附（**不是惯性**：知道要去哪一页，用固定时长滑过去）
        if let Some(slide) = self.emoji_page_slide.as_mut() {
            // 与别处不一样：`Slide` 给的是**绝对位置**，直接盖上就行
            self.emoji_page_scroll = slide.step(dt);
            if slide.finished() {
                self.emoji_page_slide = None;
            }
            self.mark_keyboard_dirty();
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
    let mut annotation = Vec::new();
    if let Some(reading) = &candidate.reading {
        annotation.push((reading.clone(), Tone::Gloss));
    }
    if let Some(translation) = &candidate.translation {
        for (i, sense) in translation.senses().iter().enumerate() {
            if i > 0 || !annotation.is_empty() {
                annotation.push((" · ".to_owned(), Tone::Faint));
            }
            if let Some(pos) = sense.part_of_speech {
                annotation.push((format!("{pos} "), Tone::Faint));
            }
            // 生词用强调色。**现在恒假**——那要词汇记录（`with_vocabulary_tracker`），
            // 属于另一条线，这轮只把画法接对。
            let tone = if sense.fresh {
                Tone::Fresh
            } else {
                Tone::Gloss
            };
            for segment in sense.furigana() {
                annotation.push((segment.text, tone));
                if let Some(reading) = segment.reading {
                    annotation.push((format!("({reading})"), Tone::Faint));
                }
            }
        }
    }
    Row {
        index: (index + 1).to_string(),
        text: candidate.text.clone(),
        annotation,
        cloud: candidate.kind == CandidateKind::Cloud,
    }
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
