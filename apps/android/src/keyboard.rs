//! 键盘这台前台：自己画键盘位图、自己算命中、自己记「哪根手指按着哪个键」。
//!
//! 单独成一层是为了**换得掉**。`Session` 只管引擎与候选条，键盘长什么样、触摸怎么算
//! 全在这里；将来若要改用安卓原生控件拼键盘（每个键一个 View），动的就是这一个文件——
//! `Session` 把这个字段置 `None`，位图那条路自然断掉。
//!
//! 分工：`Session::touch` 把**键盘那半边**的触摸转进来（坐标已减掉候选条高度），
//! 这里只回答「抬起来时兑现的是哪个键」；翻成动作在 `crate::action`，执行在 `Session`。

use qingjian_render::{
    InputMode, Key, KeyHit, KeyId, KeyboardLayout, KeyboardState, KeyboardTheme, Rendered,
    RenderedKeyboard, Renderer, ShiftState,
};

use crate::surface;
use crate::touch::{MotionAction, TOUCH_SLOP, within_slop};

/// 手指离开按下那点这么远（点），就算「在键上滑了一下」——兑现键帽角上那个小字。
///
/// **不看方向**：四个方向都算，手指往哪歪都行，不用瞄准。以前只认往下滑，
/// 得特意朝下瞄，快打时很容易滑歪。
///
/// 键高 42 点上下，这个阈值约合三分之一——正常敲字时手指只挪几个像素，到不了；
/// 特意滑一下就过。
pub(crate) const SWIPE: f32 = 12.0;

/// 尺寸与外观。壳在 `Session::configure` 时给一份。
#[derive(Debug, Clone, Copy)]
struct Metrics {
    /// 输入视图的宽度（点）。
    width: f32,

    /// 屏幕密度（点 → 像素）。命中阈值按它换算。
    density: f32,

    /// 屏幕底部被系统手势条 / 导航栏占掉的高度（点）。键要往上让开这一段。
    bottom_inset: f32,

    /// 深色主题。
    dark: bool,
}

impl Default for Metrics {
    fn default() -> Self {
        Self {
            width: 0.0,
            density: 1.0,
            bottom_inset: 0.0,
            dark: false,
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

    /// 那一根手指**已经下滑取角标了**：这一下最终打出的是角标那个字符，不是键帽上印的字。
    ///
    /// 气泡照它画——不然按住 `y` 往下滑，气泡写着 `y`、打出来却是 `6`，气泡在骗人。
    pressed_hint: Option<char>,

    /// 画好的键预览气泡，以及它是**给哪个键、多大尺寸**画的。
    ///
    /// 按住键那一下要弹；同一个键按着不动就不必重画（画一次 ~0.8ms，每拍重画白费）。
    popup: Option<Rendered>,
    popup_for: Option<(KeyId, u32, u32)>,
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
            pressed_hint: None,
            popup: None,
            popup_for: None,
        }
    }

    /// 壳报告尺寸与明暗。
    ///
    /// 脏标记由这里自己管：几项都没变就不重画。`Session` 那边不必再判一次——
    /// 各判各的，省得谁忘了同步谁。
    pub fn set_metrics(&mut self, width: f32, density: f32, bottom_inset: f32, dark: bool) {
        if (self.metrics.width - width).abs() > 0.5
            || (self.metrics.density - density).abs() > 0.01
            || (self.metrics.bottom_inset - bottom_inset).abs() > 0.5
            || self.metrics.dark != dark
        {
            self.metrics = Metrics {
                width,
                density,
                bottom_inset,
                dark,
            };
            self.dirty = true;
        }
    }

    /// 键盘占多高（点），不含底部让开的那一段。由布局决定，与屏幕尺寸无关。
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
    /// 返回抬起来时兑现的那个键；没有就是 `None`。
    pub fn touch(&mut self, action: MotionAction, pointer: i32, x: f32, y: f32) -> Option<KeyId> {
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
                    repeated: false,
                });
                self.refresh_pressed();
                None
            }
            MotionAction::Move => {
                let hit = self.hit(x, y);
                let threshold = SWIPE * self.metrics.density;
                if let Some(press) = self
                    .presses
                    .iter_mut()
                    .find(|press| press.pointer == pointer)
                {
                    // 离开按下那点够远就是「要打角标那个字符」——**四个方向都算**。
                    // 判定了就不再改回去：手指滑到键外面也还算数，这是手势不是点击。
                    let (dx, dy) = (x - press.at.0, y - press.at.1);
                    if !press.hinted
                        && press.hint.is_some()
                        && dx * dx + dy * dy >= threshold * threshold
                    {
                        press.hinted = true;
                    }
                    // 键很大（三十多点宽），手指抖一抖不该掉字，所以「还落在这个键上」就一直算按着；
                    // 滑到别的键或键之间的缝上才取消。候选条那边不是这个判法，得挪出触摸阈值。
                    if !press.hinted && (press.key.is_none() || hit != press.key) {
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
                // 下滑出来的字符走 `Literal`：与数字页、符号页同一个身份，
                // 翻成动作、全角与否都走已经有的那条路
                if ended.hinted {
                    return ended.hint.map(KeyId::Literal);
                }
                // 按住连发过的，抬手不再补一下——不然后面总是多删一个字
                if ended.repeated {
                    return None;
                }
                // 抬起时只要还在那个键上、或者只挪了触摸阈值那么点距离，都算这一下按着了
                match ended.key {
                    Some(key)
                        if !ended.sliding
                            && (self.hit(x, y) == Some(key)
                                || within_slop(ended.at, self.touch_slop(), x, y)) =>
                    {
                        Some(key)
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
        let key = match self.pressed_hint {
            Some(hint) => Key::new(KeyId::Literal(hint), key.units().max(1.0)),
            None => key,
        };

        let mark = (
            key.id,
            rect.width.round() as u32,
            rect.height.round() as u32,
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
                        &key,
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

    /// 这根手指此刻**按住**的键。没按在键上、已经滑开、或者这一下是下滑取角标，都是 `None`。
    ///
    /// 给长按连发用：连发要问的是「这根手指现在还按着哪个键」，
    /// 而不是 [`Self::pressed`] 那个「最后按下的是哪个键」——后者是给键帽上色用的，
    /// 两根手指交替时它会被后按下的那根挤掉。
    pub fn held(&self, pointer: i32) -> Option<KeyId> {
        self.presses
            .iter()
            .find(|press| press.pointer == pointer && !press.sliding && !press.hinted)
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

    /// 气泡此刻是**按哪个身份**画的——测试用，验证下滑之后画的是角标而不是字母。
    #[cfg(test)]
    pub(crate) fn popup_id(&self) -> Option<KeyId> {
        self.popup_for.map(|(id, _, _)| id)
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
        if self.pressed != key || self.pressed_hint != hint {
            self.pressed = key;
            self.pressed_hint = hint;
            self.dirty = true;
        }
    }

    /// 手指离按下那点这么近（像素）就算没挪窝。
    fn touch_slop(&self) -> f32 {
        TOUCH_SLOP * self.metrics.density
    }

    /// 当前该用的键盘主题。
    fn theme(&self) -> KeyboardTheme {
        if self.metrics.dark {
            KeyboardTheme::dark()
        } else {
            KeyboardTheme::light()
        }
    }
}

impl Default for Keyboard {
    fn default() -> Self {
        Self::new()
    }
}
