//! 触摸：按下 / 移动 / 抬起，判断落在哪个键上。
//!
//! 命中测试在渲染器一侧（`RenderedKeyboard::hit`），这里只做按钮语义那点事：
//! 按下记键、滑出去算取消、抬起的坐标必须与按下时命中的是同一个键才算数。
//! **动作映射（按了字母要干什么）不在这里**，在 `action` 那一层——渲染器只认键的身份。

/// 一次触摸的动作，与安卓 `MotionEvent` 的 `actionMasked` 对应。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionAction {
    Down,
    Move,
    Up,
    Cancel,
}

impl MotionAction {
    /// 把安卓送来的 `actionMasked` 翻译过来；不认识的当抬起处理（最保险，只会少做事）。
    pub fn from_motion(action: i32) -> Self {
        match action {
            0 => Self::Down,
            2 => Self::Move,
            1 => Self::Up,
            _ => Self::Cancel,
        }
    }
}

/// 手指移开按下那点超过这个距离（**点**，用时乘屏幕密度）就不算这一下按着了。
///
/// 8 点对的是安卓自己的 `ViewConfiguration.getScaledTouchSlop()`（8 dp）。**别拿它当像素用**：
/// 密度 2.75 的机器上 8 像素只有 2.9 点，快敲时手指挪几个像素就会被误判成滑动，整下敲击丢掉。
pub const TOUCH_SLOP: f32 = 8.0;
