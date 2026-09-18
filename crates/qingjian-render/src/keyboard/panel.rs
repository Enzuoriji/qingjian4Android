//! 键盘的几页。
//!
//! 一页一套布局、四行占满同一个高度——**页与页之间只有键换了，键盘本身不高不矮**，
//! 否则每切一次页都会把上面的应用内容顶一下（候选条固定高度就是为的这个）。

/// 键盘的一页。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Panel {
    /// 字母页。
    Letters,

    /// 数字与运算符页。
    Digits,

    /// 符号页。
    Symbols,
}
