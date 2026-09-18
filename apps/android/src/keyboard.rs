//! 键盘这台前台：自己画键盘位图、自己算命中、自己记「哪根手指按着哪个键」。
//!
//! 单独成一层是为了**换得掉**。`Session` 只管引擎与候选条，键盘长什么样、触摸怎么算
//! 全在这里；将来若要改用安卓原生控件拼键盘（每个键一个 View），动的就是这一个文件——
//! `Session` 把这个字段置 `None`，位图那条路自然断掉。
//!
//! 分工：`Session::touch` 把**键盘那半边**的触摸转进来（坐标已减掉候选条高度），
//! 这里只回答「抬起来时兑现的是哪个键」；翻成动作在 `crate::action`，执行在 `Session`。

#[cfg(test)]
use qingjian_render::KeyHit;
use qingjian_render::{
    InputMode, KeyId, KeyboardLayout, KeyboardState, KeyboardTheme, RenderedKeyboard, Renderer,
    ShiftState,
};

use crate::surface;
use crate::touch::{MotionAction, TOUCH_SLOP, within_slop};

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
        self.dirty = true;
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
                });
                self.refresh_pressed();
                None
            }
            MotionAction::Move => {
                let hit = self.hit(x, y);
                if let Some(press) = self
                    .presses
                    .iter_mut()
                    .find(|press| press.pointer == pointer)
                {
                    // 键很大（三十多点宽），手指抖一抖不该掉字，所以「还落在这个键上」就一直算按着；
                    // 滑到别的键或键之间的缝上才取消。候选条那边不是这个判法，得挪出触摸阈值。
                    if press.key.is_none() || hit != press.key {
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
    /// 给测试取点位用——真机上的点位是手指给的，不走这里。
    #[cfg(test)]
    pub(crate) fn key_rect(&self, id: KeyId) -> Option<KeyHit> {
        self.rendered
            .as_ref()?
            .keys
            .iter()
            .find(|key| key.id == id)
            .copied()
    }

    /// 命中哪个键。落在键之间的缝上、或者还没画过时是 `None`。
    fn hit(&self, x: f32, y: f32) -> Option<KeyId> {
        self.rendered
            .as_ref()
            .and_then(|keyboard| keyboard.hit(x, y))
    }

    /// 把「有没有键按着」记下来，只影响键帽的颜色。
    ///
    /// 多根手指同时按着时取**最后按下**的那根——键帽只画得出一个按下态，
    /// 而这已经够用：反馈要的是「我这一下碰到了」，不是同时高亮好几格。
    fn refresh_pressed(&mut self) {
        let key = self
            .presses
            .iter()
            .rev()
            .find_map(|press| match (press.sliding, press.key) {
                (false, Some(key)) => Some(key),
                _ => None,
            });
        if self.pressed != key {
            self.pressed = key;
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
