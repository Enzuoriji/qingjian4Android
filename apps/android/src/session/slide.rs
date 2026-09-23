//! 松手之后那一段收尾：**从当前位置滑到指定的整页**。
//!
//! 与 [`super::fling`] 那套惯性**不是一回事**：惯性是「以起手速度衰减下去」，停在哪由曲线
//! 决定；这里是「知道要去哪、用固定时长滑过去」——ViewPager2 松手后那一段就是这个。
//! 拿惯性去凑整页会差一截（起手速度与目标位置对不上），所以才单开一个类型。
//!
//! 曲线走 ease-out cubic：起手快、收尾慢，落定那一下不突兀。时长固定，**不按距离变**——
//! 拖了半页和拖了一页多都是这个时长，手感才一致（ViewPager2 的 smooth scroll 也是这个量级）。
//!
//! 节拍与 [`super::fling`] 共用：壳按 ~16ms 敲帧、问一句「现在到哪儿了」。

/// 一段吸附动画走多久（毫秒）。
const DURATION: f32 = 200.0;

/// 一次「滑到指定位置」。
#[derive(Debug, Clone, Copy)]
pub(super) struct Slide {
    /// 起手在哪。
    from: f32,

    /// 要滑到哪。
    to: f32,

    /// 已经过去多久（毫秒）。
    elapsed: f32,
}

impl Slide {
    /// 起一段从 `from` 到 `to` 的动画；两头一样时给 `None`——没什么可滑的，
    /// 调用方直接落定就行（不然会白敲 200ms 的帧）。
    pub(super) fn new(from: f32, to: f32) -> Option<Self> {
        (from != to).then_some(Self {
            from,
            to,
            elapsed: 0.0,
        })
    }

    /// 过去 `dt` 毫秒之后的**位置**。
    ///
    /// 返回的是「现在在哪」而不是「这一拍走多少」——与 [`super::fling::Fling::step`] 的接口
    /// **不一样**，那边是增量、这边是绝对位置。绝对位置才好在半路改目标（手指又按下来时）。
    ///
    /// `dt` 是壳那一拍实际过去多久，所以掉帧了也按真实时间走、不会慢动作。
    pub(super) fn step(&mut self, dt: f32) -> f32 {
        if dt > 0.0 {
            self.elapsed += dt;
        }
        self.from + (self.to - self.from) * ease_out(self.progress())
    }

    /// 走完了没有。
    pub(super) fn finished(&self) -> bool {
        self.progress() >= 1.0
    }

    /// 走到几成了（0 到 1）。
    fn progress(&self) -> f32 {
        (self.elapsed / DURATION).clamp(0.0, 1.0)
    }
}

/// ease-out cubic：`1 − (1 − t)³`。起手快、收尾慢。
fn ease_out(t: f32) -> f32 {
    let left = 1.0 - t;
    1.0 - left * left * left
}

#[cfg(test)]
mod tests {
    use super::{DURATION, Slide, ease_out};

    #[test]
    fn the_two_ends_are_where_they_should_be() {
        let mut slide = Slide::new(10.0, 50.0).expect("两头不一样，该起一段");
        assert_eq!(slide.step(0.0), 10.0, "还没走，还在起手那儿");
        let end = slide.step(DURATION);
        assert!((end - 50.0).abs() < 0.001, "走完了该正好到 50，实际 {end}");
        assert!(slide.finished());
    }

    #[test]
    fn it_ends_exactly_on_target_and_stays() {
        let mut slide = Slide::new(0.0, 100.0).expect("该起一段");
        // 一帧一帧走到底
        let mut at = 0.0;
        for _ in 0..40 {
            at = slide.step(16.0);
        }
        assert!(slide.finished(), "40 帧 × 16ms = 640ms，早该走完了");
        assert!((at - 100.0).abs() < 0.001, "落定要落在目标上，实际 {at}");
        // 走完之后再怎么敲也停在目标上，不会越过去
        assert!((slide.step(100.0) - 100.0).abs() < 0.001);
    }

    #[test]
    fn a_zero_or_negative_step_does_not_move_the_clock() {
        let mut slide = Slide::new(0.0, 10.0).expect("该起一段");
        assert_eq!(slide.step(0.0), 0.0);
        assert_eq!(slide.step(-5.0), 0.0, "负的 dt 不该倒着走");
        assert!(!slide.finished(), "空转不该算走完");
        assert!(slide.step(16.0) > 0.0);
    }

    #[test]
    fn no_slide_when_there_is_nowhere_to_go() {
        assert!(Slide::new(3.0, 3.0).is_none(), "两头一样就没什么可滑的");
    }

    /// 时长固定：拖了半页和拖了一页多，走完的时间一样——手感才一致。
    #[test]
    fn the_duration_does_not_depend_on_the_distance() {
        let mut near = Slide::new(0.0, 10.0).expect("该起一段");
        let mut far = Slide::new(0.0, 1000.0).expect("该起一段");
        let half = DURATION / 2.0;
        near.step(half);
        far.step(half);
        assert!(!near.finished() && !far.finished(), "一半时间都还没走完");
        near.step(half);
        far.step(half);
        assert!(near.finished() && far.finished(), "同样长的时长该一起走完");
    }

    /// 曲线要**起手快、收尾慢**：前半段走的路该比后半段多。
    #[test]
    fn it_leaves_early_and_arrives_gently() {
        let half = ease_out(0.5);
        assert!(
            (0.5..0.95).contains(&half),
            "一半时间该走过一半以上（先快后慢），实际 {half}"
        );
        assert_eq!(ease_out(0.0), 0.0);
        assert_eq!(ease_out(1.0), 1.0);
    }
}
