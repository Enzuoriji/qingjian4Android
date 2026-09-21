//! 候选条甩出去之后的那一段滑行（惯性）。
//!
//! 手指离开时带子还带着速度，让它自己滑一段再慢慢停下——「顺畅」的手感全在这。
//! 速度由壳量（安卓自带 `VelocityTracker`），**衰减曲线在这**：壳只管按 ~16ms 敲帧、
//! 问一句「这一拍走多少」。与长按连发、移光标同一个分工（节拍在壳、手感在 Rust）。
//!
//! 衰减走指数 `v(t) = v₀·k^t`，一步的位移按**积分**算而不是 `v·dt`——
//! 那样帧率一变滑的距离就变了（60Hz 与 50Hz 差出一截），积分出来的总距离只跟起手速度有关。

/// 起手速度低于这个（像素/毫秒）不算「甩」，手指停在哪就是哪。
///
/// 200 像素/秒：慢慢拖到一半松手不该自己接着跑。
const MIN_START: f32 = 0.2;

/// 慢到这个速度（像素/毫秒）就停下——再滑也是挪不到一个像素。
const STOP: f32 = 0.05;

/// 每毫秒衰减到原来的这么多：`0.998^1000 ≈ 0.13`，一秒后只剩一成多。
///
/// 总滑行距离是起手速度的 `1 ÷ 0.002` 倍（约 500 倍，像素/毫秒为单位）：
/// 轻轻一甩（0.5）滑 250 点上下，用力甩（3）滑一千五百来点、约两屏。
const DECAY_PER_MS: f32 = 0.998;

/// 一次甩动。单位一律是**像素/毫秒**（与 [`Session::scroll_by`] 的位移同向同单位）。
///
/// [`Session::scroll_by`]: super::Session::scroll_by
#[derive(Debug, Clone, Copy)]
pub(super) struct Fling {
    /// 此刻的速度。正数 = 带子往后滚（看后面的候选）。
    velocity: f32,
}

impl Fling {
    /// 按起手速度起一段滑行；太慢就当没甩（`None`）。
    pub(super) fn new(velocity: f32) -> Option<Self> {
        (velocity.abs() >= MIN_START).then_some(Self { velocity })
    }

    /// 过去 `dt` 毫秒，返回这一步该挪多少像素。
    ///
    /// 位移是速度的积分：`∫v₀·k^t dt = v₀·(k^dt − 1) ÷ ln k`（两个负数相除得正）。
    /// `dt` 是壳那一拍实际过去多久，所以掉帧了也走够距离、不慢动作。
    pub(super) fn step(&mut self, dt: f32) -> f32 {
        if dt <= 0.0 {
            return 0.0;
        }
        let left = DECAY_PER_MS.powf(dt);
        let moved = self.velocity * (left - 1.0) / DECAY_PER_MS.ln();
        self.velocity *= left;
        moved
    }

    /// 慢下来了没有。
    pub(super) fn finished(&self) -> bool {
        self.velocity.abs() < STOP
    }
}

#[cfg(test)]
mod tests {
    use super::{DECAY_PER_MS, Fling, MIN_START, STOP};

    #[test]
    fn a_slow_lift_is_not_a_fling() {
        assert!(
            Fling::new(MIN_START * 0.9).is_none(),
            "慢慢拖着松手不该接着滑"
        );
        assert!(Fling::new(MIN_START).is_some(), "刚够线就算甩");
        // 反着甩一样算甩
        assert!(Fling::new(-MIN_START).is_some());
    }

    #[test]
    fn it_slows_down_and_stops() {
        let mut fling = Fling::new(3.0).expect("该甩起来");
        let mut moved = 0.0;
        let mut frames = 0;
        while !fling.finished() {
            moved += fling.step(16.0);
            frames += 1;
            assert!(frames < 600, "十秒还没停，衰减写错了");
        }
        // 3 像素/毫秒的起手：总距离约 3 ÷ 0.002 = 1500 点
        assert!(
            (600.0..2400.0).contains(&moved),
            "用力甩一把该滑一千多点，实际 {moved}"
        );
        assert!(frames > 10, "不该一两帧就停，实际 {frames} 帧");
    }

    /// **帧率变了，滑的距离不该变**：这一条是「按积分算」而不是 `v·dt` 的理由。
    #[test]
    fn the_distance_does_not_depend_on_the_frame_rate() {
        let mut fast = Fling::new(2.0).expect("该甩起来");
        let mut slow = Fling::new(2.0).expect("该甩起来");
        let mut a = 0.0;
        let mut b = 0.0;
        while !fast.finished() {
            a += fast.step(8.0);
        }
        while !slow.finished() {
            b += slow.step(32.0);
        }
        assert!(
            (a - b).abs() < 20.0,
            "8ms 一帧与 32ms 一帧该滑得差不多：{a} vs {b}"
        );
    }

    #[test]
    fn a_zero_or_negative_step_moves_nothing() {
        let mut fling = Fling::new(2.0).expect("该甩起来");
        assert_eq!(fling.step(0.0), 0.0);
        assert_eq!(fling.step(-5.0), 0.0);
        // 空转不该把速度磨掉
        assert!(fling.step(16.0) > 0.0);
    }

    #[test]
    fn the_decay_is_the_advertised_one() {
        // 一秒之后只剩一成多一点
        let left = DECAY_PER_MS.powf(1000.0);
        assert!(
            (0.10..0.16).contains(&left),
            "一秒后该剩一成多，实际 {left}"
        );
    }

    /// 停下的那条线要比起手的线低不少——不然刚一甩起来就算「已经停了」，一帧都不走。
    ///
    /// 拿变量比而不是直接比两个常数：直接比是常量表达式，clippy 会拦（`assertions_on_constants`）。
    #[test]
    fn the_stop_line_is_far_below_the_start_line() {
        let (stop, start) = (STOP, MIN_START);
        assert!(
            stop * 3.0 < start,
            "停下的线（{stop}）该比起手的线（{start}）低不少"
        );
    }
}
