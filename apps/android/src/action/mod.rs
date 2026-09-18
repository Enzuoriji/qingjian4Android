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

/// 碰到一个键该干什么。
pub fn on_key(key: KeyId) -> Act {
    match key {
        KeyId::Letter(c) => Act::Push(c),
        KeyId::Space => Act::CommitHighlighted,
        KeyId::Enter => Act::CommitRaw,
        KeyId::Backspace => Act::Backspace,
        KeyId::Shift => Act::ToggleShift,
        KeyId::Mode => Act::ToggleMode,
        // 键盘上画的是全角「，」，这里给引擎的是半角原字符，转不转由它按设置定
        KeyId::Comma => Act::Punctuate(','),
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
        assert_eq!(on_key(KeyId::Letter('a')), Act::Push('a'));
    }

    #[test]
    fn the_editing_keys_map_to_their_own_actions() {
        assert_eq!(on_key(KeyId::Space), Act::CommitHighlighted);
        assert_eq!(on_key(KeyId::Enter), Act::CommitRaw);
        assert_eq!(on_key(KeyId::Backspace), Act::Backspace);
    }

    #[test]
    fn the_mode_key_toggles_rather_than_pushing_something() {
        assert_eq!(on_key(KeyId::Mode), Act::ToggleMode);
    }

    #[test]
    fn the_comma_key_hands_the_engine_the_half_width_character() {
        // 键帽上画的是「，」，但引擎拿到的该是半角 —— 全角与否是它的判断
        assert_eq!(on_key(KeyId::Comma), Act::Punctuate(','));
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
