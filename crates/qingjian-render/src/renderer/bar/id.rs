//! 候选条上可点的东西。

/// 候选条上的一个命中目标。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarHitId {
    /// 本页第几个候选，**页内下标**（从 0 起）。跨页的下标由壳自己换算。
    Candidate(usize),

    /// 上一页。
    PagePrev,

    /// 下一页。
    PageNext,

    /// 清空拼音。
    Clear,

    /// 左边那个齿轮：开 / 收「工具」页（剪贴板这一类，以后的设置也在那儿）。
    ///
    /// **只有没组句时才画**——组句时这一块整个让给候选条（见 [`Renderer::bar_height`]）。
    ///
    /// [`Renderer::bar_height`]: super::Renderer::bar_height
    Tools,
}
