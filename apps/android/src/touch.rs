//! 触摸：按下 / 移动 / 抬起，判断落在哪个键上。
//!
//! 命中测试在渲染器一侧（`RenderedKeyboard::hit`），这里只做按钮语义那点事：
//! 按下记键、滑出去算取消、抬起的坐标必须与按下时命中的是同一个键才算数。
//! **动作映射（按了字母要干什么）不在这里**，在 `action` 那一层——渲染器只认键的身份。
//!
//! **多根手指是按 pointer 分开算的**：安卓把 `POINTER_DOWN` / `POINTER_UP` 单独发出来，
//! 一次只指某一根手指。两只拇指快速交替时接触时间会重叠，只记一个「当前按下的键」
//! 会让后按下的那根把前一根挤掉，两根的字母一起丢。

/// 一次触摸的动作，与安卓 `MotionEvent` 的 `actionMasked` 对应。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionAction {
    /// 第一根手指按下。
    Down,

    /// 又一根手指按下（此时已有一根按着）。
    PointerDown,

    Move,

    /// 抬起的是已有的一根（还有别的按着）。
    PointerUp,

    /// 最后一根手指抬起。
    Up,

    Cancel,
}

impl MotionAction {
    /// 把安卓送来的 `actionMasked` 翻译过来。
    ///
    /// 5 / 6 是 `POINTER_DOWN` / `POINTER_UP`——**不是取消**，别把整盘按下状态清掉，
    /// 那正是快打掉字母的原因。真不认识的（含 `CANCEL`）才当取消。
    pub fn from_motion(action: i32) -> Self {
        match action {
            0 => Self::Down,
            5 => Self::PointerDown,
            2 => Self::Move,
            6 => Self::PointerUp,
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

/// `(x, y)` 离按下那点 `at` 还在阈值 `slop` 之内吗。
///
/// 键与候选条都用它判「这一下算不算按着了」，所以放在这儿而不是各写一份——
/// 阈值那点事（见 [`TOUCH_SLOP`]）踩过一次坑，不该有第二个版本。
pub fn within_slop(at: (f32, f32), slop: f32, x: f32, y: f32) -> bool {
    (x - at.0).abs() <= slop && (y - at.1).abs() <= slop
}

#[cfg(test)]
mod tests {
    use super::MotionAction;

    #[test]
    fn pointer_events_are_not_cancels() {
        // 5 / 6 是「还有一根手指按下 / 抬起」，不是取消。
        // 归到取消会让两根手指互相吃掉对方（快打掉字母）。
        assert_eq!(MotionAction::from_motion(5), MotionAction::PointerDown);
        assert_eq!(MotionAction::from_motion(6), MotionAction::PointerUp);
    }

    #[test]
    fn only_cancel_means_cancel() {
        assert_eq!(MotionAction::from_motion(0), MotionAction::Down);
        assert_eq!(MotionAction::from_motion(1), MotionAction::Up);
        assert_eq!(MotionAction::from_motion(2), MotionAction::Move);
        assert_eq!(MotionAction::from_motion(3), MotionAction::Cancel);
        // 不认识的（比如 4 OUTSIDE）也当取消，最保险
        assert_eq!(MotionAction::from_motion(4), MotionAction::Cancel);
    }
}
