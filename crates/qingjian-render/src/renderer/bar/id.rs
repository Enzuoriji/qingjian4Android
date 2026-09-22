//! 候选条上可点的东西。

/// 候选条上的一个命中目标。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarHitId {
    /// 本页第几个候选，**页内下标**（从 0 起）。跨页的下标由壳自己换算。
    Candidate(usize),

    /// 高亮那个候选底下那行译文里**第几条**译文（从 0 起）——点它上屏那条译文本身，
    /// 而不是候选词。
    ///
    /// 一行里可能摆着两条译文（`int. hello · int. hi`），**各是各的靶子**：
    /// 点哪个词上屏哪个。分隔符（` · `）、词性那些不归任何一条，点它们不响应。
    Translation(usize),

    /// 上一页。
    PagePrev,

    /// 下一页。
    PageNext,

    /// 清空拼音。
    Clear,

    /// 左边那个标：开 / 收「工具」页（剪贴板这一类，以后的设置也在那儿）。
    ///
    /// **只有没组句时才画**——组句时这一块整个让给候选条（见 [`Renderer::bar_height`]）。
    ///
    /// [`Renderer::bar_height`]: super::Renderer::bar_height
    Tools,
}
