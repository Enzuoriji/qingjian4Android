//! 一次输入法会话：持有 [`Engine`] 与自绘渲染器，把 Kotlin 侧的调用翻译成它们的方法。

use std::path::Path;

use qingjian_core::Engine;
use qingjian_dictionary::Dictionary;
use qingjian_render::{
    FontLibrary, KeyId, KeyboardLayout, KeyboardState, KeyboardTheme, RenderedKeyboard, Renderer,
    ShiftState, Theme,
};

use crate::error::SessionError;
use crate::probe;
use crate::surface;
use crate::touch::{MotionAction, TOUCH_SLOP};

/// 返回给 Kotlin 的位掩码：哪些面变了、有没有话要交给应用。跨语言只传数字。
pub mod flags {
    // 四个值一起构成与 Kotlin 侧 `QingjianNative` 的约定，缺一个就对不上号，所以现在就写全。
    // 候选条与上屏是 M2 / M3 的事，Rust 这边暂时只有 KEYBOARD 有人读。
    #![allow(dead_code)]

    /// 候选条变了，重新取位图。
    pub const BAR: i32 = 1;

    /// 键盘变了，重新取位图。
    pub const KEYBOARD: i32 = 2;

    /// 有文本要上屏，取走再 `commitText`。
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

    /// 键盘的宽度（点），壳在尺寸变化时告知。
    keyboard_width: f32,

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

    /// 按下时命中的键与坐标。抬起时要靠它们判断手指还在不在同一个键上。
    pressed: Option<KeyId>,
    pressed_at: (f32, f32),

    /// 最近一次「按下又抬起」落到的键（M1 调试用，接上引擎后删）。
    last_touched: Option<KeyId>,
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
            keyboard_width: 0.0,
            density: 1.0,
            bottom_inset: 0.0,
            dark: false,
            layout: KeyboardLayout::letters(),
            state: KeyboardState::default(),
            keyboard: None,
            keyboard_dirty: true,
            pressed: None,
            pressed_at: (0.0, 0.0),
            last_touched: None,
        })
    }

    /// 敲入一个字符。
    pub fn push(&mut self, c: char) {
        self.engine.push(c);
    }

    /// 清空缓冲区。
    pub fn clear(&mut self) {
        self.engine.clear();
    }

    /// 当前候选的文本，调试阶段用来验证链路。
    pub fn candidates(&self) -> Vec<String> {
        match self.engine.query() {
            Ok(query) => query
                .candidates
                .items
                .iter()
                .map(|c| c.text.clone())
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    /// 壳报告键盘的可用宽度（点）、屏幕密度、底部被系统占掉的高度、明暗，
    /// 返回键盘**总共该有多高**（点，已含底部那一段）。
    ///
    /// 高度要回传是因为安卓只按视图量出来的尺寸给输入法窗口大小——壳得知道自己该占多高，
    /// 否则窗口会被撑满整屏。几项都没变时不重画。
    pub fn configure_keyboard(
        &mut self,
        width: f32,
        density: f32,
        bottom_inset: f32,
        dark: bool,
    ) -> f32 {
        let density = if density > 0.0 { density } else { 1.0 };
        let bottom_inset = bottom_inset.max(0.0);
        if (self.keyboard_width - width).abs() > 0.5
            || (self.density - density).abs() > 0.01
            || (self.bottom_inset - bottom_inset).abs() > 0.5
            || self.dark != dark
        {
            self.keyboard_width = width;
            self.density = density;
            self.bottom_inset = bottom_inset;
            self.dark = dark;
            self.keyboard_dirty = true;
        }
        self.keyboard_theme().height + self.bottom_inset
    }

    /// 当前该用的键盘主题。
    fn keyboard_theme(&self) -> KeyboardTheme {
        if self.dark {
            KeyboardTheme::dark()
        } else {
            KeyboardTheme::light()
        }
    }

    /// 键盘的位图（8 字节头 + 预乘 RGBA）。没配过宽度或渲染器不可用时返回空。
    pub fn keyboard_surface(&mut self) -> Vec<u8> {
        if self.keyboard_width <= 0.0 {
            return Vec::new();
        }
        if self.keyboard_dirty || self.keyboard.is_none() {
            let theme = self.keyboard_theme();
            let (scale, inset) = (self.density, self.bottom_inset);
            let rendered = self.renderer.as_mut().and_then(|renderer| {
                renderer
                    .render_keyboard(
                        &self.layout,
                        &self.state,
                        self.keyboard_width,
                        inset,
                        &theme,
                        scale,
                    )
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

    /// 一次触摸。返回 [`flags`] 的位掩码。
    ///
    /// 按钮语义：按下记键、滑出这段距离就取消、抬起时必须还落在按下那个键上才算数。
    pub fn touch(&mut self, action: MotionAction, x: f32, y: f32) -> i32 {
        let hit = self.hit(x, y);
        match action {
            MotionAction::Down => {
                self.pressed = hit;
                self.pressed_at = (x, y);
                self.set_pressed(hit);
                flags::KEYBOARD
            }
            MotionAction::Move => {
                let moved = (x - self.pressed_at.0).abs() > TOUCH_SLOP
                    || (y - self.pressed_at.1).abs() > TOUCH_SLOP;
                let held = if moved { None } else { hit };
                if moved {
                    self.pressed = None;
                }
                self.set_pressed(held);
                flags::KEYBOARD
            }
            MotionAction::Up => {
                let fired = match (self.pressed, hit) {
                    (Some(down), Some(up)) if down == up => Some(up),
                    _ => None,
                };
                self.pressed = None;
                self.set_pressed(None);
                if let Some(key) = fired {
                    self.last_touched = Some(key);
                    self.act(key);
                }
                flags::KEYBOARD
            }
            MotionAction::Cancel => {
                self.pressed = None;
                self.set_pressed(None);
                flags::KEYBOARD
            }
        }
    }

    /// 最近一次按下又抬起的键的调试名称（M1 临时件）。
    pub fn last_touched_name(&self) -> String {
        self.last_touched
            .map_or_else(String::new, |key| format!("{key:?}"))
    }

    /// 改按下态，变了才标脏——省掉没必要的重画。
    fn set_pressed(&mut self, pressed: Option<KeyId>) {
        if self.state.pressed != pressed {
            self.state.pressed = pressed;
            self.keyboard_dirty = true;
        }
    }

    /// 命中哪个键。还没画过键盘就返回 `None`。
    fn hit(&self, x: f32, y: f32) -> Option<KeyId> {
        self.keyboard
            .as_ref()
            .and_then(|keyboard| keyboard.hit(x, y))
    }

    /// 按下某个键该干什么。
    ///
    /// 本轮（M1）只记账，不改引擎也不改状态——先把「点得准不准」验出来。
    /// 接上引擎在 M3：字母喂 `Engine::push`、空格上屏高亮候选、退格删字母。
    fn act(&mut self, key: KeyId) {
        if key == KeyId::Shift {
            // 先只做视觉上的来回切换，方便看锁定态对不对
            self.state.shift = match self.state.shift {
                ShiftState::Off => ShiftState::Locked,
                _ => ShiftState::Off,
            };
            self.keyboard_dirty = true;
        }
    }

    /// 位图通路的探针（M0 临时件）：`which` 0 是色块、其余是文字。返回能过 JNI 的字节串，
    /// 画不出来（渲染器没建起来、字体挂了）时返回空。
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
