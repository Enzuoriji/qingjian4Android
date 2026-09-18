//! 一次输入法会话：持有 [`Engine`] 与自绘渲染器，把 Kotlin 侧的调用翻译成它们的方法。

#[cfg(test)]
mod tests;

use std::path::Path;

use qingjian_core::{Candidate, CandidateKind, Engine, MarkedKind};
use qingjian_dictionary::Dictionary;
use qingjian_render::{
    BarHitId, FontLibrary, Frame, KeyId, KeyboardLayout, KeyboardState, KeyboardTheme, Preedit,
    PreeditSegment, PreeditStyle, RenderedBar, RenderedKeyboard, Renderer, Row, ShiftState, Theme,
};

use crate::action::{self, Act, Command};
use crate::error::SessionError;
use crate::probe;
use crate::surface;
use crate::touch::{MotionAction, TOUCH_SLOP};

/// 候选条一页画几个。
///
/// 桌面一页 9 个，手机上放不下：360pt 宽的屏幕里一页 6 个时，三字词会被截成「你…」，
/// 5 个才留得下格与格之间的缝。触摸目标也因此一样大，手指点的时候不用瞄。
const PAGE_SIZE: usize = 5;

/// 在候选条上横向划这么远（点）算翻页。
const SWIPE_MIN: f32 = 40.0;

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
}

/// 触摸落在了哪一块上：键盘的键，还是候选条上的某个目标。
///
/// 两块面的位图各自独立，但**触摸坐标是整块输入视图的**（候选条在上、键盘在下），
/// 命中测试由 [`Session`] 统一按 y 分派——壳不需要知道两块面各自在哪。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Hit {
    Key(KeyId),

    Bar(BarHitId),
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

    /// 键盘布局，建一次就够。
    layout: KeyboardLayout,

    /// 键盘此刻的样子。
    state: KeyboardState,

    /// 画好待用的键盘。命中矩形就在它里面，触摸时直接用，不必重画。
    keyboard: Option<RenderedKeyboard>,

    /// 键盘脏了没有——`keyboard_surface` 被调用时才真重画。
    keyboard_dirty: bool,

    /// 拼音行。没在组句时为 `None`。
    preedit: Option<Preedit>,

    /// 引擎给的候选，**跨页的完整列表**（`page` 只影响画哪一段）。
    candidates: Vec<Candidate>,

    /// 当前页，从 0 起。
    page: usize,

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

    /// 按下时命中的目标与坐标。抬起时要靠它们判断手指还在不在同一个目标上、是不是在划。
    pressed: Option<Hit>,
    pressed_at: (f32, f32),

    /// 这一按是不是落在候选条上（划动翻页只认从候选条起手的）。
    pressed_in_bar: bool,

    /// 最近一次「按下又抬起」碰到的目标（M3 调试用，M4 删）。
    last_touched: Option<Hit>,
}

impl Session {
    /// 打开词库、建好引擎与渲染器。`locale` 决定中日同形字取哪家字形（`zh-CN` / `ja`）。
    pub fn open(dictionary_path: &Path, locale: &str) -> Result<Self, SessionError> {
        let dictionary = Dictionary::from_path(dictionary_path)?;
        let renderer = match FontLibrary::system(locale) {
            Ok(library) => Some(Renderer::new(library)),
            Err(error) => {
                tracing::error!(%error, locale, "字体库建不起来，自绘渲染器不可用");
                None
            }
        };

        Ok(Self {
            engine: Engine::new(dictionary),
            renderer,
            width: 0.0,
            density: 1.0,
            bottom_inset: 0.0,
            dark: false,
            layout: KeyboardLayout::letters(),
            state: KeyboardState::default(),
            keyboard: None,
            keyboard_dirty: true,
            preedit: None,
            candidates: Vec::new(),
            page: 0,
            frame: Frame::default(),
            bar: None,
            bar_dirty: true,
            preedit_dirty: true,
            pending_commit: None,
            pending_commands: Vec::new(),
            pressed: None,
            pressed_at: (0.0, 0.0),
            pressed_in_bar: false,
            last_touched: None,
        })
    }

    /// 敲入一个字符（调试通路用，正常走 [`Self::touch`]）。
    pub fn push(&mut self, c: char) {
        self.engine.push(c);
        self.recompose();
    }

    /// 清空缓冲区（调试通路用）。
    pub fn clear(&mut self) {
        self.engine.clear();
        self.recompose();
    }

    /// 当前候选的文本，调试阶段用来验证链路。
    pub fn candidates(&self) -> Vec<String> {
        self.candidates.iter().map(|c| c.text.clone()).collect()
    }

    /// 壳报告输入视图的宽度（点）、屏幕密度、底部被系统占掉的高度、明暗，
    /// 返回整块输入视图**总共该有多高**（点）：候选条 + 键盘 + 底部让开的那一段。
    ///
    /// 高度要回传是因为安卓只按视图量出来的尺寸给输入法窗口大小——壳得知道自己该占多高，
    /// 否则窗口会被撑满整屏。几项都没变时不重画。
    pub fn configure(&mut self, width: f32, density: f32, bottom_inset: f32, dark: bool) -> f32 {
        let density = if density > 0.0 { density } else { 1.0 };
        let bottom_inset = bottom_inset.max(0.0);
        if (self.width - width).abs() > 0.5
            || (self.density - density).abs() > 0.01
            || (self.bottom_inset - bottom_inset).abs() > 0.5
            || self.dark != dark
        {
            self.width = width;
            self.density = density;
            self.bottom_inset = bottom_inset;
            self.dark = dark;
            self.keyboard_dirty = true;
            self.bar_dirty = true;
        }
        self.bar_height() + self.keyboard_theme().height + self.bottom_inset
    }

    /// 候选条该有多高（点）。固定值，与有没有候选无关。
    pub fn bar_height(&self) -> f32 {
        Renderer::bar_height(&self.theme())
    }

    /// 当前该用的候选窗主题。
    fn theme(&self) -> Theme {
        if self.dark {
            Theme::dark()
        } else {
            Theme::light()
        }
    }

    /// 当前该用的键盘主题。
    fn keyboard_theme(&self) -> KeyboardTheme {
        if self.dark {
            KeyboardTheme::dark()
        } else {
            KeyboardTheme::light()
        }
    }

    /// 候选条的位图（8 字节头 + 预乘 RGBA）。没配过宽度或渲染器不可用时返回空。
    pub fn bar_surface(&mut self) -> Vec<u8> {
        if self.width <= 0.0 {
            return Vec::new();
        }
        if self.bar_dirty || self.bar.is_none() {
            let theme = self.theme();
            let rendered = self.renderer.as_mut().and_then(|renderer| {
                renderer
                    .render_bar(&self.frame, self.width, &theme, self.density)
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

    /// 键盘的位图（8 字节头 + 预乘 RGBA）。没配过宽度或渲染器不可用时返回空。
    pub fn keyboard_surface(&mut self) -> Vec<u8> {
        if self.width <= 0.0 {
            return Vec::new();
        }
        if self.keyboard_dirty || self.keyboard.is_none() {
            let theme = self.keyboard_theme();
            let (scale, inset) = (self.density, self.bottom_inset);
            let rendered = self.renderer.as_mut().and_then(|renderer| {
                renderer
                    .render_keyboard(&self.layout, &self.state, self.width, inset, &theme, scale)
                    .ok()
            });
            if rendered.is_none() {
                return Vec::new();
            }
            self.keyboard = rendered;
            self.keyboard_dirty = false;
        }

        self.keyboard.as_ref().map_or_else(Vec::new, |keyboard| {
            surface::encode(&keyboard.rendered.pixmap)
        })
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

    /// 一次触摸（坐标是整块输入视图的）。返回 [`flags`] 的位掩码。
    ///
    /// 按钮语义：按下记目标、滑出这段距离就取消、抬起时必须还落在按下那个目标上才算数。
    /// 从候选条起手横向划得够远则翻页（往左划是下一页，与翻书一个方向）。
    pub fn touch(&mut self, action: MotionAction, x: f32, y: f32) -> i32 {
        let hit = self.hit(x, y);
        match action {
            MotionAction::Down => {
                self.pressed = hit;
                self.pressed_at = (x, y);
                self.pressed_in_bar = y < self.bar_pixels();
                self.set_pressed(hit);
            }
            MotionAction::Move => {
                let moved = (x - self.pressed_at.0).abs() > TOUCH_SLOP
                    || (y - self.pressed_at.1).abs() > TOUCH_SLOP;
                if moved {
                    self.pressed = None;
                }
                self.set_pressed(if moved { None } else { hit });
            }
            MotionAction::Up => {
                let fired = match (self.pressed, hit) {
                    (Some(down), Some(up)) if down == up => Some(up),
                    _ => None,
                };
                let swiped = fired.is_none() && self.pressed_in_bar;
                let delta = x - self.pressed_at.0;
                self.pressed = None;
                self.pressed_in_bar = false;
                self.set_pressed(None);
                if let Some(target) = fired {
                    self.last_touched = Some(target);
                    self.act(target);
                } else if swiped && delta.abs() > self.swipe_min() {
                    self.apply(Act::Page(if delta < 0.0 { 1 } else { -1 }));
                }
            }
            MotionAction::Cancel => {
                self.pressed = None;
                self.pressed_in_bar = false;
                self.set_pressed(None);
            }
        }
        self.mask()
    }

    /// 最近一次按下又抬起碰到的东西的调试名称（M3 临时件，M4 删）。
    pub fn last_touched_name(&self) -> String {
        self.last_touched.map_or_else(String::new, |hit| match hit {
            Hit::Key(key) => format!("{key:?}"),
            Hit::Bar(id) => format!("{id:?}"),
        })
    }

    /// 这一轮下来哪些面要重取、有没有话要交给应用。
    fn mask(&self) -> i32 {
        let mut mask = 0;
        if self.keyboard_dirty {
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
        mask
    }

    /// 改按下态，变了才标脏——省掉没必要的重画。
    ///
    /// 候选条上的按下态这一轮不画，所以只有键会传下去。
    fn set_pressed(&mut self, hit: Option<Hit>) {
        let key = match hit {
            Some(Hit::Key(key)) => Some(key),
            _ => None,
        };
        if self.state.pressed != key {
            self.state.pressed = key;
            self.keyboard_dirty = true;
        }
    }

    /// 候选条在整块输入视图里占的高度（像素）。键盘接在它下面。
    fn bar_pixels(&self) -> f32 {
        self.bar_height() * self.density
    }

    /// 划多远算翻页（像素）。
    fn swipe_min(&self) -> f32 {
        SWIPE_MIN * self.density
    }

    /// 触摸落到了哪一块。
    ///
    /// 候选条在上、键盘在下，两块面共用一条 y 轴：落在候选条那一段的交给它，
    /// 剩下的减掉候选条高度再交给键盘。
    fn hit(&self, x: f32, y: f32) -> Option<Hit> {
        let bar_pixels = self.bar_pixels();
        if y < bar_pixels {
            return self
                .bar
                .as_ref()
                .and_then(|bar| bar.hit(x, y))
                .map(Hit::Bar);
        }
        self.keyboard
            .as_ref()
            .and_then(|keyboard| keyboard.hit(x, y - bar_pixels))
            .map(Hit::Key)
    }

    /// 碰到了某个目标：先翻成动作，再执行。
    fn act(&mut self, target: Hit) {
        match target {
            Hit::Key(key) => {
                if let Some(act) = action::on_key(key) {
                    self.apply(act);
                }
            }
            Hit::Bar(id) => self.apply(action::on_bar(id)),
        }
    }

    /// 执行一个动作。引擎在什么状态决定同一动作的不同走法，都写在这里。
    fn apply(&mut self, act: Act) {
        match act {
            Act::Push(c) => {
                // 拼音不分大小写：中文模式下 Shift 只影响键帽显示，不改变喂进去的字母
                self.engine.push(c.to_ascii_lowercase());
                self.recompose();
            }
            Act::CommitCandidate(index) => {
                let absolute = self.page * PAGE_SIZE + index;
                if let Some(candidate) = self.candidates.get(absolute).cloned() {
                    let text = self.engine.commit(&candidate);
                    self.commit_text(text);
                }
            }
            Act::CommitHighlighted => {
                // 高亮永远是本页第一个：还没有移动高亮的手势（点了就直接上屏）
                let first = self.page * PAGE_SIZE;
                match self.candidates.get(first).cloned() {
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
                self.state.shift = match self.state.shift {
                    ShiftState::Off => ShiftState::Locked,
                    _ => ShiftState::Off,
                };
                self.keyboard_dirty = true;
            }
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

    /// 缓冲变了：重查候选，回到第一页。
    fn recompose(&mut self) {
        self.page = 0;
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
        self.refresh();
        self.preedit_dirty = true;
    }

    /// 用当前的候选与页码重建待画的那一帧。
    ///
    /// 翻页走这条路而不是 [`Self::recompose`]——重查会把页码打回第一页。
    fn refresh(&mut self) {
        self.frame = self.build_frame();
        self.bar_dirty = true;
    }

    /// 本页画哪几个候选、页码是几。这一轮不画译文，所以不调 `engine.annotate()`。
    fn build_frame(&self) -> Frame {
        let start = self.page * PAGE_SIZE;
        let rows: Vec<Row> = self
            .candidates
            .iter()
            .skip(start)
            .take(PAGE_SIZE)
            .enumerate()
            .map(|(i, candidate)| row(i, candidate))
            .collect();
        let pages = self.page_count();
        Frame {
            preedit: self.preedit.clone(),
            // 高亮永远是本页第一个：还没有移动高亮的手势
            highlighted: (!rows.is_empty()).then_some(0),
            // 只有一页就不报页码——与桌面一致
            footer: (pages > 1).then(|| format!("{}/{}", self.page + 1, pages)),
            rows,
            sentence: None,
            status: None,
        }
    }

    /// 一共有几页，至少 1。
    fn page_count(&self) -> usize {
        self.candidates.len().div_ceil(PAGE_SIZE).max(1)
    }

    /// 翻页，夹在首末页之间。
    fn turn_page(&mut self, step: isize) {
        let pages = self.page_count();
        if pages <= 1 {
            return;
        }
        let target = (self.page as isize + step).clamp(0, pages as isize - 1) as usize;
        if target != self.page {
            self.page = target;
            self.engine.note_page_turn();
            self.refresh();
        }
    }

    /// 位图通路的探针（M0 临时件，收尾时删）：`which` 0 是色块、其余是文字。
    /// 返回能过 JNI 的字节串，画不出来（渲染器没建起来、字体挂了）时返回空。
    pub fn probe(&mut self, which: i32) -> Vec<u8> {
        let pixmap = match which {
            0 => Some(probe::colors()),
            _ => self.renderer.as_mut().and_then(|r| probe::text(r).ok()),
        };
        pixmap.map_or_else(Vec::new, |pixmap| surface::encode(&pixmap))
    }

    /// 探针用：报告[探针文字](probe::SAMPLE)的每个字实际落到了哪个字族（M0 临时件）。
    ///
    /// 字体出问题时靠它定位：中文落成日文面、emoji 找不到字体，都会在这里显出来。
    pub fn trace(&mut self) -> Vec<String> {
        self.renderer
            .as_mut()
            .map(|renderer| renderer.trace_families(probe::SAMPLE, &Theme::light()))
            .unwrap_or_default()
    }
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
