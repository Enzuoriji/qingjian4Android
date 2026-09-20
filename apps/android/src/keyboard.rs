//! 键盘这台前台：自己画键盘位图、自己算命中、自己记「哪根手指按着哪个键」。
//!
//! 单独成一层是为了**换得掉**。`Session` 只管引擎与候选条，键盘长什么样、触摸怎么算
//! 全在这里；将来若要改用安卓原生控件拼键盘（每个键一个 View），动的就是这一个文件——
//! `Session` 把这个字段置 `None`，位图那条路自然断掉。
//!
//! 分工：`Session::touch` 把**键盘那半边**的触摸转进来（坐标已减掉候选条高度），
//! 这里只回答「抬起来时兑现的是哪个键」；翻成动作在 `crate::action`，执行在 `Session`。

use qingjian_render::{
    InputMode, Key, KeyHit, KeyId, KeyboardLayout, KeyboardState, KeyboardTheme, Popup, Rendered,
    RenderedKeyboard, Renderer, ShiftState,
};

use crate::surface;
use crate::touch::{MotionAction, TOUCH_SLOP, within_slop};

/// 手指从按下那点挪够这么远（点），就算「这是一记手势，不是点击」。
///
/// 取角标只认**往下**滑、⌫ 清空只认**往上**滑——两处比的都是**纵向**位移，
/// 所以这个阈值该跟**键高**比，不是跟键宽比。键高竖屏上有 42~55 点（跟着屏幕自适应，
/// 见 `KeyboardTheme::fitted`），半步就有 21~27 点，22 落在**键内**：
/// 手势认出来之前，这一下都还算「按着这个键」。
///
/// **为什么只留往下**（2026-09-20 改，原先四个方向都算）：真机上快打会误蹦符号。
/// 而横向那条路**结构上救不了**——「手指出了键 = 这一下作废」与阈值线是**重合**的
/// （都卡在半个键宽，约 15 点），阈值往上加只会变成「符号不出、字母也一起丢」。
/// 收回成一个方向之后两件事同时成立：①误触面砍到四分之一（只有**朝下滚**才可能撞上）；
/// ②阈值改成跟键高比，才有余量从 16 提到 22。
/// 代价是另外三个方向的角标只能去 `123` / 符 面板打——那些字符面板里都有。
///
/// 手感常数，只能靠真机调。判据：快打一段不蹦符号 = 行；还蹦就再往上加。
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
const CLEAR_SWIPE: f32 = SWIPE;

/// ⌫ 上往上滑之后，气泡上写这句话。
///
/// 键上那个退格图标说不出「松手会怎样」——滑上去之后这一下已经不是「删一个字」了，
/// 气泡得把话讲清楚，不然用户不知道松手会发生什么。
const CLEAR_HINT: &str = "松手清空";

/// 抬起来时兑现的东西。
///
/// 大多数时候是「按了某个键」，但空格键上横着滑是**移光标**——那不是某个键，
/// 翻成动作也就不是 [`crate::action::on_key`] 那条路，得分开报。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fired {
    /// 按了某个键。
    Key(KeyId),

    /// 光标往右（正数）或往左（负数）移几格。
    MoveCursor(isize),

    /// 把光标**前面整段**清掉。⌫ 上往上滑、松手时兑现。
    ///
    /// 清多少由壳去问应用（输入法不知道光标前面有什么），这边只说「清」。
    ClearToStart,
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

    /// 手指已经滑开了，这一下不再算「点击」。
    ///
    /// **不能直接把记录删掉**——删了抬起时就不知道刚才是从哪个键按下去的，
    /// 也就判不出「滑出去又滑回来」这一下还算不算。留个标记就够。
    sliding: bool,

    /// 这个键下滑能打出来的字符（键帽角上那个小字）。没有角标就是 `None`。
    hint: Option<char>,

    /// 已经往下滑够远了，这一下兑现的是角标那个字符。
    ///
    /// 与 [`Self::sliding`] 是两回事：下滑是**手势**，手指离开这个键照样算数；
    /// `sliding` 说的是「点击作废」。
    hinted: bool,

    /// 这根手指此刻**横着离开了按下那点多远**（点，正数往右）。只有空格上移光标用得上。
    swipe: f32,

    /// 已经越过死区、进入移光标了——那这一下就不是「按空格」。
    cursor_started: bool,

    /// 移光标的小数累加器：速度是小数（慢的时候几拍才够一格），攒够一格才走。
    cursor_carry: f32,

    /// 已经往上滑够了：松手要把光标前面整段清掉。
    clearing: bool,

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

    /// 那一根手指**已经下滑取角标了**：这一下最终打出的是角标那个字符，不是键帽上印的字。
    ///
    /// 气泡照它画——不然按住 `y` 往下滑，气泡写着 `y`、打出来却是 `6`，气泡在骗人。
    pressed_hint: Option<char>,

    /// 画好的键预览气泡，以及它是**给哪个键、多大尺寸**画的。
    ///
    /// 按住键那一下要弹；同一个键按着不动就不必重画（画一次 ~0.8ms，每拍重画白费）。
    popup: Option<Rendered>,
    popup_for: Option<(KeyId, u32, u32, bool)>,
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
            pressed_hint: None,
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
    /// 键盘自己只记「哪个键看着是按下的」。
    pub fn surface(
        &mut self,
        renderer: Option<&mut Renderer>,
        shift: ShiftState,
        mode: InputMode,
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
                    hinted: false,
                    swipe: 0.0,
                    cursor_started: false,
                    cursor_carry: 0.0,
                    clearing: false,
                    repeated: false,
                });
                self.refresh_pressed();
                None
            }
            MotionAction::Move => {
                let hit = self.hit(x, y);
                let threshold = SWIPE * self.metrics.density;
                let dead_zone = CURSOR_DEAD_ZONE * self.metrics.density;
                let clear_swipe = CLEAR_SWIPE * self.metrics.density;
                if let Some(press) = self
                    .presses
                    .iter_mut()
                    .find(|press| press.pointer == pointer)
                {
                    // 取角标**只认往下滑**（2026-09-20 改，原先四个方向都算；为什么见 [`SWIPE`]）。
                    // 判定了就不再改回去：手指滑到键外面也还算数，这是手势不是点击。
                    let dy = y - press.at.1;
                    if !press.hinted && press.hint.is_some() && dy >= threshold {
                        press.hinted = true;
                        // 之前因为「手指出了键」记下的作废，到这儿一笔勾销——手势既然认出来了，
                        // 这一下就照角标兑现。不清掉的话 `refresh_pressed` 会把手指排除在外，
                        // 气泡和键帽按下态会在手势认出的那一刻**当场消失**（`clearing` 踩过同一个坑）。
                        press.sliding = false;
                    }
                    // ⌫ 上**往上滑** = 要清掉光标前面整段（**松手才清**，滑上去只是「预备」）。
                    // 挑往上而不是往左：往左是「退格」本来的方向，容易跟普通退格混；
                    // 往上是个独立的动作，不会误触。
                    if press.key == Some(KeyId::Backspace) && dy <= -clear_swipe {
                        press.clearing = true;
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
                    // - 取角标（`hinted`）：手势本来就是滑出去
                    // - 移光标（`cursor_started`）：空格键宽，划着划着就出去了，那不是「这一下不要了」
                    // - ⌫ 上滑清空（`clearing`）：**这条是补的**——往上一滑手指就出了 ⌫，
                    //   原先这里没排掉它，于是 `sliding` 一置上，`refresh_pressed` 就不认这根手指了，
                    //   气泡当场消失——用户正是靠气泡上那句「松手清空」才知道自己在干什么
                    if !press.hinted
                        && !press.cursor_started
                        && !press.clearing
                        && (press.key.is_none() || hit != press.key)
                    {
                        press.sliding = true;
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
                let ended = index.map(|index| self.presses.remove(index))?;
                self.refresh_pressed();
                // 滑出来的字符走 `Literal`：与数字页、符号页同一个身份，
                // 翻成动作、全角与否都走已经有的那条路
                if ended.hinted {
                    return ended.hint.map(|hint| Fired::Key(KeyId::Literal(hint)));
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
                    return Some(Fired::ClearToStart);
                }
                // 抬起时只要还在那个键上、或者只挪了触摸阈值那么点距离，都算这一下按着了
                match ended.key {
                    Some(key)
                        if !ended.sliding
                            && (self.hit(x, y) == Some(key)
                                || within_slop(ended.at, self.touch_slop(), x, y)) =>
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
        // **这一下会打出什么就画什么**：下滑取角标时，兑现的是角标那个字符。
        // 画成 `Literal` 与真正兑现时走的是同一个身份，气泡上的字与打出来的字必然一致
        let shown = match self.pressed_hint {
            Some(hint) => Key::new(KeyId::Literal(hint), key.units().max(1.0)),
            None => key,
        };
        // ⌫ 上往上滑之后这一下不是「删一个字」了，气泡改说「松手清空」——
        // 退格图标说不出「松手会怎样」
        let content = if self.pressed_clearing {
            Popup::Text(CLEAR_HINT)
        } else {
            Popup::Key(&shown)
        };

        let mark = (
            shown.id,
            rect.width.round() as u32,
            rect.height.round() as u32,
            self.pressed_clearing,
        );
        if self.popup_for != Some(mark) || self.popup.is_none() {
            let theme = self.theme();
            let state = KeyboardState {
                shift,
                mode,
                pressed: self.pressed,
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
    /// **手势一开就不算「按住」**：滑动取角标、空格移光标、退格上滑清空，这三样都是从
    /// 「按住这个键」岔出去的路。不排掉的话，滑得慢一点（超过连发门槛 400ms）
    /// 就会一边做手势一边被连发删——实测踩到过。
    pub fn held(&self, pointer: i32) -> Option<KeyId> {
        self.presses
            .iter()
            .find(|press| {
                press.pointer == pointer
                    && !press.sliding
                    && !press.hinted
                    && !press.cursor_started
                    && !press.clearing
            })
            .and_then(|press| press.key)
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

    /// 气泡此刻是**按哪个身份**画的——测试用，验证下滑之后画的是角标而不是字母。
    #[cfg(test)]
    pub(crate) fn popup_id(&self) -> Option<KeyId> {
        self.popup_for.map(|(id, _, _, _)| id)
    }

    /// 命中哪个键。落在键之间的缝上、或者还没画过时是 `None`。
    fn hit(&self, x: f32, y: f32) -> Option<KeyId> {
        self.rendered
            .as_ref()
            .and_then(|keyboard| keyboard.hit(x, y))
    }

    /// 这个位置上的键，往下滑能打出什么字符。键没有角标（或没命中键）就是 `None`。
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
    fn refresh_pressed(&mut self) {
        let held = self
            .presses
            .iter()
            .rev()
            .find(|press| !press.sliding && press.key.is_some());
        let key = held.and_then(|press| press.key);
        // 下滑取角标的那一根：这一下兑现的是角标，不是键帽上的字
        let hint = held
            .filter(|press| press.hinted)
            .and_then(|press| press.hint);
        let clearing = held.is_some_and(|press| press.clearing);
        if self.pressed != key || self.pressed_hint != hint || self.pressed_clearing != clearing {
            self.pressed = key;
            self.pressed_hint = hint;
            self.pressed_clearing = clearing;
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
