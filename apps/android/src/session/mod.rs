//! 一次输入法会话：持有 [`Engine`] 与自绘渲染器，把 Kotlin 侧的调用翻译成它们的方法。
//!
//! 这里只管**引擎与候选条**。键盘的画法、命中与按下状态在 [`Keyboard`] 里，
//! 触摸进来按 y 分给两边（见 [`Session::touch`]）——换掉键盘那半边不影响这一层。

#[cfg(test)]
mod tests;

use std::path::Path;

use qingjian_core::{Candidate, CandidateKind, EmojiTable, Engine, MarkedKind};
use qingjian_dictionary::Dictionary;
use qingjian_render::{
    BarHitId, FontLibrary, Frame, InputMode, KeyboardLayout, Panel, Preedit, PreeditSegment,
    PreeditStyle, RenderedBar, Renderer, Row, ShiftState, Theme,
};

use crate::action::{self, Act, Command};
use crate::error::SessionError;
use crate::keyboard::{Fired, Keyboard};
use crate::surface;
use crate::touch::{MotionAction, TOUCH_SLOP, within_slop};

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
            landscape: false,
            keyboard: Some(Keyboard::new()),
            shift: ShiftState::default(),
            mode: InputMode::default(),
            panel: Panel::Letters,
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
    }

    /// 壳报告输入视图的宽度（点）、屏幕密度、底部被系统占掉的高度、明暗，
    /// 返回整块输入视图**总共该有多高**（点）：候选条 + 键盘 + 底部让开的那一段。
    ///
    /// 高度要回传是因为安卓只按视图量出来的尺寸给输入法窗口大小——壳得知道自己该占多高，
    /// 否则窗口会被撑满整屏。几项都没变时不重画。
    pub fn configure(
        &mut self,
        width: f32,
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
        }
        if let Some(keyboard) = self.keyboard.as_mut() {
            keyboard.set_metrics(width, density, bottom_inset, dark, landscape);
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

    /// 候选条的位图（8 字节头 + 预乘 RGBA）。没配过宽度、渲染器不可用、**或者没在组句**时返回空。
    ///
    /// 没在组句时把 `bar` 也清掉：那块命中区跟着一起没了，触摸自然落不到候选条上。
    /// 壳收到空字节串要把视图上那张位图**撤掉**，视图量出来的高度才会跟着缩回去——
    /// 光不更新是不够的，那张旧位图还占着位置。
    pub fn bar_surface(&mut self) -> Vec<u8> {
        if self.width <= 0.0 {
            return Vec::new();
        }
        if !self.composing() {
            self.bar = None;
            self.bar_dirty = false;
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

    /// 按住键时那张预览气泡的位图（8 字节头 + 预乘 RGBA）。没在预览时是空数组。
    ///
    /// 与候选条一样，**空表示「这个小窗现在不该在」**——壳收到空字节串要把浮动小窗收起来。
    pub fn popup_surface(&mut self) -> Vec<u8> {
        let (shift, mode) = (self.shift, self.mode);
        match self.keyboard.as_mut() {
            Some(keyboard) => keyboard.popup_surface(self.renderer.as_mut(), shift, mode),
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
        match self.keyboard.as_mut() {
            Some(keyboard) => keyboard.surface(self.renderer.as_mut(), shift, mode),
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
        let bar_pixels = self.bar_pixels();
        let fired = self
            .keyboard
            .as_mut()
            .and_then(|keyboard| keyboard.touch(action, pointer, x, y - bar_pixels));
        match fired {
            Some(Fired::Key(key)) => self.apply(action::on_key(key)),
            Some(Fired::MoveCursor(steps)) => self.move_cursor(steps),
            Some(Fired::ClearToStart) => self.clear_to_start(),
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
    pub fn repeat(&mut self, pointer: i32) -> i32 {
        let key = self.keyboard.as_mut().and_then(|keyboard| {
            let key = keyboard.held(pointer).filter(|key| action::repeats(*key))?;
            // 记一笔，抬起时就不再按「点击」补一下了
            keyboard.note_repeat(pointer);
            Some(key)
        });
        if let Some(key) = key {
            self.apply(action::on_key(key));
        }
        self.mask()
    }

    /// 候选条那半边：按下记一笔、滑出去算取消、抬起时判是点了候选还是划着翻页。
    ///
    /// 判法与键盘不同：候选格横向拖是翻页手势，所以只按「挪没挪出触摸阈值」判，
    /// 不按「还在不在原来那一格上」判——否则拖一下会被当成点了那个候选。
    /// 从候选条起手横向划得够远则翻页（往左划是下一页，与翻书一个方向）。
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
                    sliding: false,
                });
            }
            MotionAction::Move => {
                let slop = self.touch_slop();
                if let Some(held) = self.pressed.iter_mut().find(|held| held.pointer == pointer)
                    && !within_slop(held.at, slop, x, y)
                {
                    held.sliding = true;
                }
            }
            MotionAction::Up | MotionAction::PointerUp => {
                let index = self.pressed.iter().position(|held| held.pointer == pointer);
                let Some(ended) = index.map(|index| self.pressed.remove(index)) else {
                    return;
                };
                let hit = self.bar.as_ref().and_then(|bar| bar.hit(x, y));
                let fired = match ended.hit {
                    Some(id)
                        if !ended.sliding
                            && (hit == Some(id)
                                || within_slop(ended.at, self.touch_slop(), x, y)) =>
                    {
                        Some(id)
                    }
                    _ => None,
                };
                if let Some(id) = fired {
                    self.apply(action::on_bar(id));
                } else if (x - ended.at.0).abs() > self.swipe_min() {
                    // 往左划是下一页，与翻书一个方向
                    self.apply(Act::Page(if x < ended.at.0 { 1 } else { -1 }));
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
