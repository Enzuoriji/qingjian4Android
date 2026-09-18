//! 按键与候选条上的点击该干什么。
//!
//! 渲染器只回「碰到了什么」（`KeyId` / `BarHitId`），**翻成动作**在这一层，**执行**在
//! [`Session::apply`](crate::session::Session)。分开是为了让按键语义集中在一处、一眼看得全，
//! 而不是散在触摸处理的各个分支里；渲染器那边也就永远不需要知道「上屏」是什么东西。
//!
//! 这一层是纯翻译：进来一个键，出去一个动作，不看引擎状态也不改任何东西，所以能单独测。

mod act;
mod command;

pub use act::Act;
pub use command::Command;

use qingjian_render::{BarHitId, KeyId};

/// 碰到一个键该干什么。`None` 是这个键还没接上（中 / 英切换留给 M4）。
pub fn on_key(key: KeyId) -> Option<Act> {
    match key {
        KeyId::Letter(c) => Some(Act::Push(c)),
        KeyId::Space => Some(Act::CommitHighlighted),
        KeyId::Enter => Some(Act::CommitRaw),
        KeyId::Backspace => Some(Act::Backspace),
        KeyId::Shift => Some(Act::ToggleShift),
        // 中 / 英切换是 M4；逗号要按「组句中进英文直输段」处理，也是 M4，先不接错
        KeyId::Mode | KeyId::Comma => None,
    }
}

/// 碰到候选条上的一块该干什么。候选给的是**页内**下标，与渲染器报的一致。
pub fn on_bar(id: BarHitId) -> Act {
    match id {
        BarHitId::Candidate(index) => Act::CommitCandidate(index),
        BarHitId::PagePrev => Act::Page(-1),
        BarHitId::PageNext => Act::Page(1),
        BarHitId::Clear => Act::Clear,
    }
}

#[cfg(test)]
mod tests {
    use super::{Act, on_bar, on_key};
    use qingjian_render::{BarHitId, KeyId};

    #[test]
    fn letters_go_to_the_engine() {
        assert_eq!(on_key(KeyId::Letter('a')), Some(Act::Push('a')));
    }

    #[test]
    fn the_three_editing_keys_map_to_their_own_actions() {
        assert_eq!(on_key(KeyId::Space), Some(Act::CommitHighlighted));
        assert_eq!(on_key(KeyId::Enter), Some(Act::CommitRaw));
        assert_eq!(on_key(KeyId::Backspace), Some(Act::Backspace));
    }

    #[test]
    fn unimplemented_keys_do_nothing_rather_than_something_wrong() {
        // 逗号在组句中要进英文直输段，接一半会让标点跑到中文前面去
        assert_eq!(on_key(KeyId::Comma), None);
        assert_eq!(on_key(KeyId::Mode), None);
    }

    #[test]
    fn bar_blocks_map_to_their_actions() {
        assert_eq!(on_bar(BarHitId::Candidate(0)), Act::CommitCandidate(0));
        assert_eq!(on_bar(BarHitId::Candidate(4)), Act::CommitCandidate(4));
        assert_eq!(on_bar(BarHitId::PagePrev), Act::Page(-1));
        assert_eq!(on_bar(BarHitId::PageNext), Act::Page(1));
        assert_eq!(on_bar(BarHitId::Clear), Act::Clear);
    }
}
