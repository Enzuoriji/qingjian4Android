//! 一次输入法会话：持有 [`Engine`] 与自绘渲染器，把 Kotlin 侧的调用翻译成它们的方法。

#[cfg(test)]
mod tests;

use std::path::Path;

use qingjian_core::{Candidate, CandidateKind, EmojiTable, Engine, MarkedKind};
use qingjian_dictionary::Dictionary;
use qingjian_render::{
    BarHitId, FontLibrary, Frame, InputMode, KeyId, KeyboardLayout, KeyboardState, KeyboardTheme,
    Preedit, PreeditSegment, PreeditStyle, RenderedBar, RenderedKeyboard, Renderer, Row,
    ShiftState, Theme,
};

use crate::action::{self, Act, Command};
use crate::error::SessionError;
use crate::surface;
use crate::touch::{MotionAction, TOUCH_SLOP};

/// 候选条一页画几个。
///
/// 桌面一页 9 个，手机上放不下：360pt 宽的屏幕里一页 6 个时，三字词会被截成「你…」，
/// 5 个才留得下格与格之间的缝。触摸目标也因此一样大，手指点的时候不用瞄。
const PAGE_SIZE: usize = 5;

/// 在候选条上横向划这么远（点）算翻页。
const SWIPE_MIN: f32 = 40.0;

/// 随包资源目录里的 emoji 字体名（`assets/emoji/README.md` 写了为什么要带它）。
const EMOJI_FONT: &str = "NotoColorEmoji.ttf";

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

    /// 此刻按着的手指们，**按根记**。
    ///
    /// 快打时两根拇指的接触时间会重叠，只留一个「当前按下的键」的话，
    /// 后按下的那根会把前一根挤掉，两根的字母一起丢——真机上报的「点快了掉字母」就是它。
    pressed: Vec<Pressed>,
}

/// 一根按着的手指。
#[derive(Debug, Clone, Copy)]
struct Pressed {
    /// 安卓给的 pointer id。
    pointer: i32,

    /// 按下时命中的目标。
    hit: Option<Hit>,

    /// 按下时的坐标。抬起时要靠它判断手指还在不在同一个目标上、是不是在划。
    at: (f32, f32),

    /// 这一按是不是落在候选条上（划动翻页只认从候选条起手的）。
    in_bar: bool,

    /// 手指已经滑开了，这一下不再算「点击」。
    ///
    /// **不能直接把记录删掉**——删了抬起时就不知道刚才从哪儿按的、划了多远，
    /// 翻页手势也就跟着没了。留个标记，抬起时按「不是点击」处理，还够判是不是在划。
    sliding: bool,
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

        Ok(Self {
            engine,
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
            pressed: Vec::new(),
        })
    }

    /// 清空缓冲区。换应用时壳调它，免得在 A 应用敲的拼音跑到 B 应用里。
    pub fn clear(&mut self) {
        self.engine.clear();
        self.recompose();
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

    /// 现在是英文模式吗。
    fn english(&self) -> bool {
        self.state.mode == InputMode::English
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

    /// 一次触摸（坐标是整块输入视图的）。`pointer` 是安卓给的 pointer id。返回 [`flags`] 的位掩码。
    ///
    /// 按钮语义，**要松**：快敲的时候手指本来就会挪几个像素，判定一紧就会把整下敲击当成滑动取消掉。所以
    ///
    /// - 手指还落在按下那个键上，就一直算按着；滑到别的键或键之间的缝上才取消
    /// - 抬起时只要还在那个键上、或者只挪了 [`TOUCH_SLOP`] 那么点距离，都算数
    ///
    /// 每一根手指各记各的（[`Pressed`]）：两根拇指快速交替时接触时间会重叠，
    /// 只留一个「当前按下的键」会让后按下的那根把前一根挤掉，两根的字母一起丢。
    ///
    /// 从候选条起手横向划得够远则翻页（往左划是下一页，与翻书一个方向）。
    pub fn touch(&mut self, action: MotionAction, pointer: i32, x: f32, y: f32) -> i32 {
        let hit = self.hit(x, y);
        match action {
            MotionAction::Down | MotionAction::PointerDown => {
                self.pressed.retain(|held| held.pointer != pointer);
                self.pressed.push(Pressed {
                    pointer,
                    hit,
                    at: (x, y),
                    in_bar: y < self.bar_pixels(),
                    sliding: false,
                });
                self.sync_pressed();
            }
            MotionAction::Move => {
                // 键与候选条判得不一样：
                // 键很大（三十多点宽），手指抖一抖不该掉字，所以「还在这个键上」就继续算按着；
                // 候选格也宽，但横向拖是翻页手势，只按「挪没挪出触摸阈值」判，
                // 否则拖一下会被当成点了那个候选。
                let slop = self.touch_slop();
                if let Some(held) = self.pressed.iter_mut().find(|held| held.pointer == pointer) {
                    let keep = match held.hit {
                        Some(Hit::Key(key)) => hit == Some(Hit::Key(key)),
                        Some(Hit::Bar(_)) => Self::within_slop(held.at, slop, x, y),
                        None => false,
                    };
                    if !keep {
                        held.sliding = true;
                    }
                }
                self.sync_pressed();
            }
            MotionAction::Up | MotionAction::PointerUp => {
                let index = self.pressed.iter().position(|held| held.pointer == pointer);
                let ended = index.map(|index| self.pressed.remove(index));
                self.sync_pressed();
                let Some(ended) = ended else {
                    return self.mask();
                };
                let fired = match ended.hit {
                    Some(down)
                        if !ended.sliding
                            && (hit == Some(down)
                                || Self::within_slop(ended.at, self.touch_slop(), x, y)) =>
                    {
                        Some(down)
                    }
                    _ => None,
                };
                if let Some(target) = fired {
                    self.act(target);
                } else if ended.in_bar && (x - ended.at.0).abs() > self.swipe_min() {
                    // 往左划是下一页，与翻书一个方向
                    self.apply(Act::Page(if x < ended.at.0 { 1 } else { -1 }));
                }
            }
            MotionAction::Cancel => {
                self.pressed.clear();
                self.sync_pressed();
            }
        }
        self.mask()
    }

    /// 把「有没有键按着」同步给键盘，只影响键帽的颜色。
    ///
    /// 多根手指同时按着时取**最后按下**的那根——键帽只画得出一个按下态，
    /// 而这已经够用：反馈要的是「我这一下碰到了」，不是同时高亮好几格。
    fn sync_pressed(&mut self) {
        let key = self
            .pressed
            .iter()
            .rev()
            .find_map(|held| match (held.sliding, held.hit) {
                (false, Some(Hit::Key(key))) => Some(key),
                _ => None,
            });
        if self.state.pressed != key {
            self.state.pressed = key;
            self.keyboard_dirty = true;
        }
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

    /// 候选条在整块输入视图里占的高度（像素）。键盘接在它下面。
    fn bar_pixels(&self) -> f32 {
        self.bar_height() * self.density
    }

    /// 划多远算翻页（像素）。
    fn swipe_min(&self) -> f32 {
        SWIPE_MIN * self.density
    }

    /// 手指离按下那点这么近（像素）就算没挪窝。**要按密度换算**：安卓自己的触摸阈值是 8 dp，
    /// 直接拿 8 像素当阈值的话，密度 2.75 的机器上只有 2.9 个点，快敲必然被误判成滑动。
    fn touch_slop(&self) -> f32 {
        TOUCH_SLOP * self.density
    }

    /// `(x, y)` 离按下那点 `at` 还在阈值 `slop` 之内吗。
    fn within_slop(at: (f32, f32), slop: f32, x: f32, y: f32) -> bool {
        (x - at.0).abs() <= slop && (y - at.1).abs() <= slop
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
            Hit::Key(key) => self.apply(action::on_key(key)),
            Hit::Bar(id) => self.apply(action::on_bar(id)),
        }
    }

    /// 执行一个动作。引擎在什么状态决定同一动作的不同走法，都写在这里。
    fn apply(&mut self, act: Act) {
        match act {
            Act::Push(c) => self.type_letter(c),
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
                // 安卓上的习惯是单击锁定、再击解锁（桌面才是按住）
                self.state.shift = match self.state.shift {
                    ShiftState::Off => ShiftState::Locked,
                    _ => ShiftState::Off,
                };
                self.keyboard_dirty = true;
            }
            Act::ToggleMode => {
                self.state.mode = match self.state.mode {
                    InputMode::Chinese => InputMode::English,
                    InputMode::English => InputMode::Chinese,
                };
                // 切换时把没上屏的拼音丢掉：留着的话，英文模式下那串字母会按英文词算候选
                self.engine.clear();
                self.engine
                    .set_english_mode(self.state.mode == InputMode::English);
                self.keyboard_dirty = true;
                self.recompose();
            }
            Act::Punctuate(c) => self.punctuate(c),
        }
    }

    /// 打一个标点。
    ///
    /// 还在组句就先把高亮候选上屏——手机上打标点应当结束当前这串拼音，「nihao」+「，」得到
    /// 「你好，」而不是把逗号插到拼音前面去。桌面的做法是把标点收进「英文直输段」（`nihao,` 整串
    /// 一起算），那是给实体键盘的，触摸键盘上不是这个预期，这里不跟。
    fn punctuate(&mut self, c: char) {
        if !self.engine.composition().is_empty()
            && let Some(candidate) = self.candidates.get(self.page * PAGE_SIZE).cloned()
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
            let c = if self.state.shift.is_upper() {
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
