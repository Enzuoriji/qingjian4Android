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
}
