//! 一次输入法会话：持有 [`Engine`] 与自绘渲染器，把 Kotlin 侧的调用翻译成它们的方法。

use std::path::Path;

use qingjian_core::{Candidate, CandidateKind, CandidateLayout, Engine, MarkedKind};
use qingjian_dictionary::Dictionary;
use qingjian_render::{
    BarHitId, FontLibrary, Frame, KeyId, KeyboardLayout, KeyboardState, KeyboardTheme, Preedit,
    PreeditSegment, PreeditStyle, RenderedBar, RenderedKeyboard, Renderer, Row, ShiftState, Theme,
};

use crate::error::SessionError;
use crate::probe;
use crate::surface;
use crate::touch::{MotionAction, TOUCH_SLOP};

/// 候选条一页画几个。
///
/// 桌面一页 9 个，手机上放不下：360pt 宽的屏幕里一页 6 个时，三字词会被截成「你…」，
/// 5 个才留得下格与格之间的缝。触摸目标也因此一样大，手指点的时候不用瞄。
const PAGE_SIZE: usize = 5;

/// 返回给 Kotlin 的位掩码：哪些面变了、有没有话要交给应用。跨语言只传数字。
pub mod flags {
    // 四个值一起构成与 Kotlin 侧 `QingjianNative` 的约定，缺一个就对不上号，所以现在就写全。
    // 上屏与拼音镜像（COMMIT / PREEDIT）是 M3 的事，Rust 这边还没人读。
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

    /// 当前该画的候选条那一帧：拼音行 + 本页候选。缓冲变化时由 [`Self::compose`] 重建。
    frame: Frame,

    /// 画好待用的候选条。
    bar: Option<RenderedBar>,

    /// 候选条脏了没有——`bar_surface` 被调用时才真重画。
    bar_dirty: bool,

    /// 按下时命中的目标与坐标。抬起时要靠它们判断手指还在不在同一个目标上。
    pressed: Option<Hit>,
    pressed_at: (f32, f32),

    /// 最近一次「按下又抬起」碰到的目标（M2 调试用，M4 删）。
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
            frame: Frame::default(),
            bar: None,
            bar_dirty: true,
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

    /// 一次触摸（坐标是整块输入视图的）。返回 [`flags`] 的位掩码。
    ///
    /// 按钮语义：按下记目标、滑出这段距离就取消、抬起时必须还落在按下那个目标上才算数。
    pub fn touch(&mut self, action: MotionAction, x: f32, y: f32) -> i32 {
        let hit = self.hit(x, y);
        match action {
            MotionAction::Down => {
                self.pressed = hit;
                self.pressed_at = (x, y);
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
                self.pressed = None;
                self.set_pressed(None);
                if let Some(target) = fired {
                    self.last_touched = Some(target);
                    self.act(target);
                }
            }
            MotionAction::Cancel => {
                self.pressed = None;
                self.set_pressed(None);
            }
        }
        self.mask()
    }

    /// 这一轮下来哪些面要重取。
    fn mask(&self) -> i32 {
        let mut mask = 0;
        if self.keyboard_dirty {
            mask |= flags::KEYBOARD;
        }
        if self.bar_dirty {
            mask |= flags::BAR;
        }
        mask
    }

    /// 最近一次按下又抬起碰到的东西的调试名称（M2 临时件，M4 删）。
    pub fn last_touched_name(&self) -> String {
        self.last_touched.map_or_else(String::new, |hit| match hit {
            Hit::Key(key) => format!("{key:?}"),
            Hit::Bar(id) => format!("{id:?}"),
        })
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

    /// 触摸落到了哪一块。
    ///
    /// 候选条在上、键盘在下，两块面共用一条 y 轴：落在候选条那一段的交给它，
    /// 剩下的减掉候选条高度再交给键盘。
    fn hit(&self, x: f32, y: f32) -> Option<Hit> {
        let bar_pixels = self.bar_height() * self.density;
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

    /// 碰到了某个目标该干什么。
    ///
    /// M2 只接字母——够敲出拼音让候选条出词。上屏、翻页、退格、清空是 M3。
    fn act(&mut self, target: Hit) {
        match target {
            Hit::Key(KeyId::Shift) => {
                // 先只做视觉上的来回切换，方便看锁定态对不对
                self.state.shift = match self.state.shift {
                    ShiftState::Off => ShiftState::Locked,
                    _ => ShiftState::Off,
                };
                self.keyboard_dirty = true;
            }
            Hit::Key(KeyId::Letter(c)) => {
                // 拼音不分大小写：中文模式下 Shift 只影响键帽显示，不改变喂进去的字母
                self.engine.push(c.to_ascii_lowercase());
                self.compose();
            }
            _ => {}
        }
    }

    /// 按引擎当前的组句状态重查候选，重建待画的那一帧。缓冲变了就调它。
    fn compose(&mut self) {
        self.frame = self.build_frame();
        self.bar_dirty = true;
    }

    /// 组一帧：拼音行 + 本页候选。缓冲空时给空帧（候选条只剩底色，高度不变）。
    fn build_frame(&mut self) -> Frame {
        if self.engine.composition().is_empty() {
            return Frame::default();
        }
        let Ok(query) = self.engine.query() else {
            // 还拼不成拼音：拼音行照画，只是没有候选
            let text = self.engine.composition().text().to_owned();
            let cursor = text.chars().count();
            return Frame {
                preedit: Some(Preedit::plain(&text, cursor)),
                ..Frame::default()
            };
        };
        let segments = query.marked_segments();
        let preedit = Preedit {
            segments: segments.iter().map(preedit_segment).collect(),
            cursor: query.marked_cursor(),
        };
        // 这一轮不接云联想，云端槽位给 0
        let layout = CandidateLayout::new(query.candidates.items, PAGE_SIZE, 0);
        let rows: Vec<Row> = layout
            .page(0)
            .iter()
            .enumerate()
            .filter_map(|(i, cell)| cell.candidate().map(|c| row(i, c)))
            .collect();
        Frame {
            preedit: Some(preedit),
            highlighted: (!rows.is_empty()).then_some(0),
            // 只有一页就不报页码——与桌面一致
            footer: (layout.pages() > 1).then(|| format!("1/{}", layout.pages())),
            rows,
            sentence: None,
            status: None,
        }
    }

    /// 位图通路的探针（M0 临时件，M2 收尾删）：`which` 0 是色块、其余是文字。
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

#[cfg(test)]
mod tests {
    use super::Session;
    use crate::touch::MotionAction;
    use qingjian_render::KeyId;
    use std::path::PathBuf;

    /// 验收用的屏幕宽（点）与密度，按一台常见手机竖屏。
    const WIDTH: f32 = 360.0;
    const DENSITY: f32 = 2.75;

    /// 词库：产品数据优先，退回随包的样例。都没有就跳过（CI 容器里可能没有产品数据）。
    fn dictionary() -> Option<PathBuf> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        [
            "data/generated/dict.qj",
            "data/generated/dict.tsv",
            "assets/sample/dict.tsv",
        ]
        .into_iter()
        .map(|rel| root.join(rel))
        .find(|path| path.is_file())
    }

    /// 某个字母键中心的坐标（**整块输入视图的像素**，与壳传进来的一致）。
    ///
    /// 布局里的字母键存的是大写（画的时候才按 Shift 转小写），所以这里先转过去比。
    fn key_centre(session: &Session, letter: char) -> (f32, f32) {
        let keyboard = session
            .keyboard
            .as_ref()
            .expect("键盘还没画过，没有命中矩形");
        let key = keyboard
            .keys
            .iter()
            .find(|key| key.id == KeyId::Letter(letter.to_ascii_uppercase()))
            .unwrap_or_else(|| panic!("键盘上没有 {letter}"));
        // y 从候选条底下开始——壳传进来的就是整块输入视图的坐标
        let bar_pixels = session.bar_height() * session.density;
        (
            key.x + key.width / 2.0,
            bar_pixels + key.y + key.height / 2.0,
        )
    }

    /// 点一下某个键：按下再抬起，落在同一点上。
    fn tap(session: &mut Session, letter: char) {
        let (x, y) = key_centre(session, letter);
        session.touch(MotionAction::Down, x, y);
        session.touch(MotionAction::Up, x, y);
    }

    fn ready() -> Option<Session> {
        let mut session = Session::open(&dictionary()?, "zh-CN").ok()?;
        session.configure(WIDTH, DENSITY, 0.0, false);
        // 两块面都画一次，命中矩形才存在
        session.keyboard_surface();
        session.bar_surface();
        Some(session)
    }

    #[test]
    fn tapping_nihao_offers_it_in_the_bar() {
        let Some(mut session) = ready() else {
            return;
        };
        for letter in ['n', 'i', 'h', 'a', 'o'] {
            tap(&mut session, letter);
        }

        let preedit = session
            .frame
            .preedit
            .as_ref()
            .expect("敲了字母，拼音行该有内容")
            .text();
        assert_eq!(preedit, "ni'hao", "拼音行该把音节用 ' 切开");

        let drawn: Vec<&str> = session
            .frame
            .rows
            .iter()
            .map(|row| row.text.as_str())
            .collect();
        assert!(
            drawn.contains(&"你好"),
            "候选条里该有「你好」，实际画的是 {drawn:?}"
        );
        assert!(
            drawn.len() <= 5,
            "一页最多 5 个，实际 {} 个：{drawn:?}",
            drawn.len()
        );
        assert_eq!(session.frame.highlighted, Some(0), "默认高亮第一个");
        assert_eq!(session.frame.rows[0].index, "1", "序号从 1 起");
    }

    #[test]
    fn the_bar_is_drawn_at_the_fixed_height() {
        let Some(mut session) = ready() else {
            return;
        };
        let bytes = session.bar_surface();
        assert!(bytes.len() > 8, "候选条该有位图");
        let height = u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        assert_eq!(
            height,
            (session.bar_height() * DENSITY).round() as u32,
            "候选条高度必须是主题定死的那个值"
        );
    }

    #[test]
    fn a_tap_on_the_bar_is_not_a_key_press() {
        let Some(mut session) = ready() else {
            return;
        };
        // 候选条那一段（键盘上方）点下去，不该往引擎里喂字母
        let (x, y) = (WIDTH * DENSITY / 2.0, session.bar_height() * DENSITY / 2.0);
        session.touch(MotionAction::Down, x, y);
        session.touch(MotionAction::Up, x, y);
        assert!(
            session.frame.preedit.is_none(),
            "候选条那一排不归任何键，不该出拼音"
        );
    }
}
