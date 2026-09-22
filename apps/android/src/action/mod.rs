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

use qingjian_render::{BarHitId, KeyId, Panel};

/// 碰到一个键该干什么。
pub fn on_key(key: KeyId) -> Act {
    match key {
        KeyId::Letter(c) => Act::Push(c),
        KeyId::Space => Act::CommitHighlighted,
        KeyId::Enter => Act::CommitRaw,
        KeyId::Backspace => Act::Backspace,
        KeyId::Shift => Act::ToggleShift,
        KeyId::Mode => Act::ToggleMode,
        // 键盘上画的是全角「，」「。」，这里给引擎的是半角原字符，转不转由它按设置定
        KeyId::Comma => Act::Punctuate(','),
        KeyId::Period => Act::Punctuate('.'),
        // 数字与符号一样：键帽上是原字符，全角与否交给引擎
        KeyId::Literal(c) => Act::Punctuate(c),
        KeyId::Panel(panel) => Act::SwitchPanel(panel),
        // 工具页那几格：0 剪贴板、1 表情、2 颜文字、3 设置（顺序见 `qingjian_render` 的 `TOOLS`）。
        // **每一格都要写出来**：剩下的那些是空格子（页里留着给以后的工具），
        // 用一个 `Tool(_)` 通配的话，点空处会莫名跳到颜文字页。
        KeyId::Tool(0) => Act::SwitchPanel(Panel::Clipboard),
        KeyId::Tool(1) => Act::SwitchPanel(Panel::Emoji),
        KeyId::Tool(2) => Act::SwitchPanel(Panel::Kaomoji),
        KeyId::Tool(3) => Act::OpenSettings,
        KeyId::Tool(_) => Act::Nothing,
        // 记录格报的是**屏幕上**第几格；它对着整份里的哪一条，是会话的事
        // （它才知道列表滚到哪儿了）
        KeyId::Clipboard(index) => Act::PasteClipboard(index),
        KeyId::ClipboardClear => Act::ClearClipboard,
        // 表情页：点一个上屏、点标签切分类、翻标签条
        KeyId::Emoji(index) => Act::Emoji(index),
        KeyId::EmojiGroup(index) => Act::EmojiGroup(index),
        KeyId::EmojiGroupPage(step) => Act::EmojiGroupPage(step),
    }
}

/// 这个键按住不放会不会连发。
///
/// **只有退格连发。** 删错一串字时一下一下点太慢，别的键按住连发都没有意义：
/// 字母键按住该出的是角标（下滑那条路），空格按住是上屏、再按住只会连着上屏，
/// 回车更不该连发。
pub fn repeats(key: KeyId) -> bool {
    matches!(key, KeyId::Backspace)
}

/// 碰到候选条上的一块该干什么。候选给的是**页内**下标，与渲染器报的一致。
pub fn on_bar(id: BarHitId) -> Act {
    match id {
        BarHitId::Candidate(index) => Act::CommitCandidate(index),
        BarHitId::Translation(sense) => Act::CommitTranslation(sense),
        BarHitId::PagePrev => Act::Page(-1),
        BarHitId::PageNext => Act::Page(1),
        BarHitId::Clear => Act::Clear,
        BarHitId::Tools => Act::ToggleTools,
    }
}

#[cfg(test)]
mod tests {
    use super::{Act, on_bar, on_key};
    use qingjian_render::{BarHitId, KeyId, Panel};

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
        // 句号一样：键帽上画全角「。」，交出去的是半角 `.`
        assert_eq!(on_key(KeyId::Period), Act::Punctuate('.'));
    }

    #[test]
    fn literals_go_through_the_punctuation_path() {
        // 数字与符号都走这条路：全角与否是引擎按设置定的，这边只报原字符
        assert_eq!(on_key(KeyId::Literal('7')), Act::Punctuate('7'));
        assert_eq!(on_key(KeyId::Literal('?')), Act::Punctuate('?'));
    }

    #[test]
    fn panel_keys_switch_panels() {
        assert_eq!(
            on_key(KeyId::Panel(Panel::Digits)),
            Act::SwitchPanel(Panel::Digits)
        );
        assert_eq!(
            on_key(KeyId::Panel(Panel::Letters)),
            Act::SwitchPanel(Panel::Letters)
        );
    }

    #[test]
    fn tool_slots_map_to_their_own_tools_and_the_empty_ones_do_nothing() {
        assert_eq!(on_key(KeyId::Tool(0)), Act::SwitchPanel(Panel::Clipboard));
        assert_eq!(on_key(KeyId::Tool(1)), Act::SwitchPanel(Panel::Emoji));
        assert_eq!(on_key(KeyId::Tool(2)), Act::SwitchPanel(Panel::Kaomoji));
        assert_eq!(on_key(KeyId::Tool(3)), Act::OpenSettings);
        // 剩下的是**空格子**（工具页留着给以后的工具）：点了什么也不该发生
        assert_eq!(on_key(KeyId::Tool(4)), Act::Nothing);
        assert_eq!(on_key(KeyId::Tool(14)), Act::Nothing);
    }

    #[test]
    fn bar_blocks_map_to_their_actions() {
        assert_eq!(on_bar(BarHitId::Candidate(0)), Act::CommitCandidate(0));
        assert_eq!(on_bar(BarHitId::Candidate(4)), Act::CommitCandidate(4));
        assert_eq!(on_bar(BarHitId::PagePrev), Act::Page(-1));
        assert_eq!(on_bar(BarHitId::PageNext), Act::Page(1));
        assert_eq!(on_bar(BarHitId::Clear), Act::Clear);
        assert_eq!(on_bar(BarHitId::Translation(0)), Act::CommitTranslation(0));
        assert_eq!(on_bar(BarHitId::Translation(1)), Act::CommitTranslation(1));
    }
}
