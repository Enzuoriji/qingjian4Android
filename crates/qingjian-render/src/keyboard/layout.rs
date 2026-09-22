//! 键盘布局：一行一行的按键。
//!
//! 几何按「单位宽」算，不写死坐标——每行按自己的总宽**居中**摆放，第 2 行（9 个键）自然
//! 得到半键错位，不用单独记缩进量。一个单位多宽由 [`KeyboardLayout::unit_width`] 取最挤的那一行定。

use super::key::{Key, KeyId, KeyWidth};
use super::panel::Panel;

/// 一行的宽度，按单位算。**各页都是 5**：数字 / 符号页是五列，工具与剪贴板页也按这个排——
/// 单位宽取最挤的那一行，所以各页的行宽一致、格子边缘对得齐。
const ROW_UNITS: f32 = 5.0;

/// 剪贴板**记录区一屏摆几条**：一条一行，五行记录 + 第六行控制。
///
/// 2026-09-21 从「三行两格、一屏六条」改成这个（用户要「仿搜狗」）：两列时一条只摊到
/// 半屏宽，十来个字就被截了，而剪贴板里多的是长句和网址——一行一条、占满整宽才认得出。
///
/// **这是视口的行数，不是一屏的上限**：条目多了不用翻页，往下拉（`clipboard_scroll`）。
pub const CLIPBOARD_CELLS: usize = 5;

/// 工具页上那几格的名字，顺序就是 [`KeyboardLayout::tools`] 的格子顺序。
///
/// 现在四格：剪贴板 / 表情 / 颜文字 / 设置。剩下那些空格子留给以后的工具（没排的不画）。
pub const TOOLS: [&str; 4] = ["剪贴板", "表情", "颜文字", "设置"];

/// 表情页的格子：一行几个、一共几行。5 × 3 = 15 个一屏。
///
/// 比剪贴板那种「一条一行」密得多：emoji 就是一个字，一格放得下；
/// 颜文字要宽一些，但共用同一份布局，靠**缩字号**塞进去（见渲染那边）。
pub const EMOJI_COLS: usize = 5;
pub const EMOJI_ROWS: usize = 3;

/// 工具页一行摆几个图标格子。一排 5 个，与别的页同一个单位宽。
const TOOLS_PER_ROW: usize = 5;

/// 工具页给工具留几格（三行，第四行留给「返回」）。
///
/// 留这么多是照搜狗那个面板的密度——它也是一屏十几个图标位，常用的那几个排在前面。
const TOOL_SLOTS: usize = TOOLS_PER_ROW * 3;

/// 一行按键，一个字符一个键——字母页与数字 / 符号页的一半都是这么来的。
fn literals(chars: &str) -> KeyRow {
    KeyRow {
        keys: chars
            .chars()
            .map(|c| Key::new(KeyId::Literal(c), 1.0))
            .collect(),
    }
}

/// 字母页那三行：字母一行，对应的角标一行。
///
/// 角标照 [fcitx5-android 的 `TextKeyboard`] 抄——它把每个键的「下滑打什么」直接写在布局里
/// （`AlphabetKey(字母, 角标)`），是现成的、被人用过的答案，比我们自己编一套强。
/// 数字行顶在字母上面；剩下两行是打标点最常用的那些，**不是** US 键盘 Shift 上档的全表
/// （那套里 `!`、`"` 这些在中文输入里用不上，占着角标反而难记）。
///
/// [fcitx5-android 的 `TextKeyboard`]: https://github.com/fcitx5-android/fcitx5-android/blob/master/app/src/main/java/org/fcitx/fcitx5/android/input/keyboard/TextKeyboard.kt
const LETTER_ROWS: [(&str, &str); 3] = [
    ("QWERTYUIOP", "1234567890"),
    ("ASDFGHJKL", "@*+-=/#()"),
    ("ZXCVBNM", "':\"?!~\\"),
];

/// 一行字母键，角标跟着一起摆。
fn letter_row(letters: &str, hints: &str) -> KeyRow {
    KeyRow {
        keys: letters
            .chars()
            .zip(hints.chars())
            .map(|(letter, hint)| Key::letter(letter, hint))
            .collect(),
    }
}

/// 一行按键。
#[derive(Debug, Clone)]
pub struct KeyRow {
    pub keys: Vec<Key>,
}

impl KeyRow {
    /// 这一行**按单位宽算**的键加起来有几个单位（不含缝隙，也不含撑满的那个键）。
    pub fn weight(&self) -> f32 {
        self.keys.iter().map(Key::units).sum()
    }

    /// 这一行有没有「撑满剩余宽度」的键。
    pub fn has_fill(&self) -> bool {
        self.keys.iter().any(|key| key.width == KeyWidth::Fill)
    }
}

/// 一整套键盘。
#[derive(Debug, Clone)]
pub struct KeyboardLayout {
    rows: Vec<KeyRow>,
}

impl KeyboardLayout {
    /// 字母页：26 键全键盘，每个字母键上都带一个角标（见 [`LETTER_ROWS`]）。
    ///
    /// 最下一行两头的键宽 1.5、中间的 1，空格是**撑满**的（[`KeyWidth::Fill`]）——
    /// 这一排只有 7 个键、比上面少 3 条缝，靠空格吃掉多出来的那一段，两头才跟第 1、3 行对齐。
    /// 隔音符号 `'` 没有独立键位，现在挂在 `Z` 的角标上——它原先住符号页，符号页改版后没了着落。
    pub fn letters() -> Self {
        let [qwerty, home, bottom] = LETTER_ROWS.map(|(letters, hints)| letter_row(letters, hints));

        Self {
            rows: vec![
                qwerty,
                home,
                KeyRow {
                    // 两头的 ⇧ 与 ⌫ 也是**撑满**的：这一排 9 个键（8 条缝），比第 1 行少一条缝，
                    // 全按单位宽排下来两头会各缩进去 7 像素。剩下的给这两个键平分，才跟第 1 行齐平
                    keys: [
                        vec![Key::fill(KeyId::Shift)],
                        bottom.keys,
                        vec![Key::fill(KeyId::Backspace)],
                    ]
                    .concat(),
                },
                KeyRow {
                    keys: vec![
                        // 字母页**直接进得了数字页与符号页**，不必先绕一层。
                        // 这一格宽 1.5：跟回车凑成一对「两头的键」，空格才正好落在正中
                        // （见下面那条注释）。它比隔壁的 `123` 宽，是居中的代价
                        Key::new(KeyId::Panel(Panel::Symbols), 1.5),
                        Key::new(KeyId::Panel(Panel::Digits), 1.0),
                        // 逗号在空格**左边**、句号在右边，中 / 英再往右——
                        // **空格左右各 3.5 个单位**，正落在这排正中
                        Key::new(KeyId::Comma, 1.0),
                        // 空格**不按单位宽**，它吃掉这一行剩下的：这一排 7 个键比上面
                        // 10 个键少 3 条缝，固定宽度排下来两头会各缩进去半个键。
                        // 剩下的都给空格，两头就跟第 1、3 行对齐了
                        Key::fill(KeyId::Space),
                        Key::new(KeyId::Period, 1.0),
                        Key::new(KeyId::Mode, 1.0),
                        Key::new(KeyId::Enter, 1.5),
                    ],
                },
            ],
        }
    }

    /// 数字页。四行五列，**按键排成计算器那样**（1-2-3 / 4-5-6 / 7-8-9，0 在下面），
    /// 左边一竖条是四个运算符（照搜狗那个面板的样子）。
    ///
    /// 每行都是 5 个单位，所以这一页的键**比字母页宽一倍**——一列一个数字，好按。
    pub fn digits() -> Self {
        Self {
            rows: vec![
                KeyRow {
                    keys: [
                        vec![Key::new(KeyId::Literal('+'), 1.0)],
                        literals("123").keys,
                        vec![Key::new(KeyId::Backspace, 1.0)],
                    ]
                    .concat(),
                },
                KeyRow {
                    keys: [
                        vec![Key::new(KeyId::Literal('-'), 1.0)],
                        literals("456").keys,
                        // 这一格原来是「返回」，它挪到底下「中」的位置去了（那儿才是回退该在的地方）
                        vec![Key::new(KeyId::Literal('@'), 1.0)],
                    ]
                    .concat(),
                },
                KeyRow {
                    keys: [
                        vec![Key::new(KeyId::Literal('*'), 1.0)],
                        literals("789").keys,
                        vec![Key::new(KeyId::Panel(Panel::Symbols), 1.0)],
                    ]
                    .concat(),
                },
                KeyRow {
                    keys: vec![
                        Key::new(KeyId::Literal('/'), 1.0),
                        // 返回在 0 前面：这页放中 / 英是多余的（打字时用不上），回退才该在这儿
                        Key::new(KeyId::Panel(Panel::Letters), 1.0),
                        Key::new(KeyId::Literal('0'), 1.0),
                        Key::new(KeyId::Space, 1.0),
                        Key::new(KeyId::Enter, 1.0),
                    ],
                },
            ],
        }
    }

    /// 符号页。也是四行五列，跟数字页一样的宽度。
    ///
    /// 键帽画的是**半角原字符**：中文模式下引擎会转成全角（`?`→`？`），英文模式下原样打出去。
    /// 画成固定的全角就会在英文模式下骗人。
    pub fn symbols() -> Self {
        Self {
            rows: vec![
                KeyRow {
                    keys: [literals("[]{}").keys, vec![Key::new(KeyId::Backspace, 1.0)]].concat(),
                },
                KeyRow {
                    keys: [
                        literals("#%^&").keys,
                        // 这页的中 / 英有用，留着；@ 这格给「回字母页」
                        vec![Key::new(KeyId::Panel(Panel::Letters), 1.0)],
                    ]
                    .concat(),
                },
                literals("_=!?."),
                KeyRow {
                    keys: vec![
                        Key::new(KeyId::Mode, 1.0),
                        Key::new(KeyId::Panel(Panel::Digits), 1.0),
                        Key::new(KeyId::Space, 1.0),
                        Key::new(KeyId::Comma, 1.0),
                        Key::new(KeyId::Enter, 1.0),
                    ],
                },
            ],
        }
    }

    /// 工具页：**一排排的图标格子**（每格 1 个单位宽，一行 5 个），最后一整行是「返回」。
    ///
    /// 照搜狗那个工具面板排的（2026-09-21 用户要「入口和搜狗一致」）：面板上是**一格一个
    /// 图标、图标下面写名字**，不是我们原先那种「一整行一个文字键」。
    /// 空格子（还没排工具的那些）不画，见 [`TOOLS`]。
    pub fn tools() -> Self {
        let mut rows: Vec<KeyRow> = (0..TOOL_SLOTS / TOOLS_PER_ROW)
            .map(|row| KeyRow {
                keys: (0..TOOLS_PER_ROW)
                    .map(|col| Key::new(KeyId::Tool(row * TOOLS_PER_ROW + col), 1.0))
                    .collect(),
            })
            .collect();
        // 「返回」撑满整行：这一行只有它一个键，按单位宽算的话两头会各缩进去一截
        // （单位宽是照上面那几行 5 个格子的排法定死的）。别的页那个「返回」是五格之一，
        // 不这样——那是**页里的一格**，不是整行。
        rows.push(KeyRow {
            keys: vec![Key::fill(KeyId::Panel(Panel::Letters))],
        });
        Self { rows }
    }

    /// 表情页：**上面一条分类标签、下面一片格子**（emoji 与颜文字共用）。
    ///
    /// 标签条是 `‹ [分类] [分类] [分类] ›` 五个 1 单位的格子——分类比这多，靠两头那两个
    /// 箭头翻（与候选条那对 `‹ ›` 一个意思）。下面三行 × 5 格 = 15 个表情，
    /// 一屏这么多；多了靠上下滑（与剪贴板那套滚动一样，下一步接）。
    pub fn emoji() -> Self {
        let mut rows = vec![KeyRow {
            keys: vec![
                Key::new(KeyId::EmojiGroupPage(-1), 1.0),
                Key::new(KeyId::EmojiGroup(0), 1.0),
                Key::new(KeyId::EmojiGroup(1), 1.0),
                Key::new(KeyId::EmojiGroup(2), 1.0),
                Key::new(KeyId::EmojiGroupPage(1), 1.0),
            ],
        }];
        for row in 0..EMOJI_ROWS {
            rows.push(KeyRow {
                keys: (0..EMOJI_COLS)
                    .map(|col| Key::new(KeyId::Emoji(row * EMOJI_COLS + col), 1.0))
                    .collect(),
            });
        }
        Self { rows }
    }

    /// 剪贴板页：**一条记录占一整行**，最后一行是控制。
    ///
    /// 记录格与别的页一样是 5 个单位一行（铺满整宽、左右边对得齐），
    /// 控制行两个 1 个单位的键窄一点居中。
    ///
    /// **记录区是能上下滚的**（2026-09-21 用户要「向下滑动选择」）：下面这几行只是**视口**，
    /// 一屏摆得下几条就写几个格子；条目多了靠 `KeyboardState::clipboard_scroll` 往下拉，
    /// 不再是「一屏几条 + 翻页按钮」。
    pub fn clipboard() -> Self {
        let mut rows: Vec<KeyRow> = (0..CLIPBOARD_CELLS)
            .map(|index| KeyRow {
                keys: vec![Key::new(KeyId::Clipboard(index), ROW_UNITS)],
            })
            .collect();
        rows.push(KeyRow {
            keys: vec![
                Key::new(KeyId::Panel(Panel::Letters), 1.0),
                Key::new(KeyId::ClipboardClear, 1.0),
            ],
        });
        Self { rows }
    }

    /// 这一页是不是表情页（emoji 与颜文字共用一份布局，所以一起认）。
    pub fn is_emoji(&self) -> bool {
        self.rows
            .iter()
            .any(|row| row.keys.iter().any(|key| matches!(key.id, KeyId::Emoji(_))))
    }

    /// 这一页是不是剪贴板页。
    ///
    /// 渲染器要知道这个：**空列表时它得在键盘中间写一句话**，不然整块键盘上只剩底下
    /// 那四个控制键，看着像坏了（搜狗也是这么写的）。
    pub fn is_clipboard(&self) -> bool {
        self.rows.iter().any(|row| {
            row.keys
                .iter()
                .any(|key| matches!(key.id, KeyId::Clipboard(_)))
        })
    }

    /// 某一页的布局。
    pub fn of(panel: Panel) -> Self {
        match panel {
            Panel::Letters => Self::letters(),
            Panel::Digits => Self::digits(),
            Panel::Symbols => Self::symbols(),
            Panel::Tools => Self::tools(),
            Panel::Clipboard => Self::clipboard(),
            // emoji 与颜文字共用一份布局（见 `Panel::Kaomoji` 的注释）
            Panel::Emoji | Panel::Kaomoji => Self::emoji(),
        }
    }

    pub fn rows(&self) -> &[KeyRow] {
        &self.rows
    }

    /// 一页的行高（点）：键盘总高去掉行间那几条缝，按行数均分。
    ///
    /// 渲染（`render_keyboard`）与安卓那边算「剪贴板一格多高」都用这个——
    /// 公式只写一遍，两边不会走样（`height` 与 `gap` 用同一套单位，点或像素都行）。
    pub fn row_height(&self, height: f32, gap: f32) -> f32 {
        let rows = self.rows().len().max(1) as f32;
        (height - gap * (rows - 1.0)) / rows
    }

    /// 在 `width` 点宽里，一个标准单位占多宽。
    ///
    /// 取**最挤的那一行**来算：任何一行的总宽都不会超过 `width`，其余行居中摆放。
    pub fn unit_width(&self, width: f32, gap: f32) -> f32 {
        self.rows
            .iter()
            // 一个固定宽度的键都没有的排不参与——一个单位多宽对它没有意义（除下来是无穷大），
            // 它整排都是「撑满」的
            .filter(|row| row.weight() > 0.0)
            .map(|row| (width - gap * (row.keys.len() - 1) as f32) / row.weight())
            .fold(f32::MAX, f32::min)
    }

    /// 这一行**按单位宽算的那部分**有多宽（点）——撑满的那个键不算在内。
    ///
    /// 没有撑满键的行，这就是整行的宽度（量出来居中用）；有撑满键的行，它铺满整宽，
    /// 这个值只是「除去撑满键还占掉多少」，撑满键自己拿的是剩下的。
    pub fn row_width(row: &KeyRow, unit: f32, gap: f32) -> f32 {
        unit * row.weight() + gap * (row.keys.len().saturating_sub(1)) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::{CLIPBOARD_CELLS, Key, KeyId, KeyWidth, KeyboardLayout, Panel, TOOLS_PER_ROW};

    /// 剪贴板页：**一条记录一行**、每行铺满整宽，控制行四个键窄一点居中。
    ///
    /// 2026-09-21 从「三行两格」改的（用户要仿搜狗）——所以这里守着「一行一个格子」，
    /// 别退回两列：两列时一条只摊到半屏宽，长句和网址十来个字就被截了。
    #[test]
    fn the_clipboard_page_has_one_entry_per_row() {
        let layout = KeyboardLayout::clipboard();
        let (width, gap) = (360.0, 8.0);
        let unit = layout.unit_width(width, gap);

        assert_eq!(
            layout.rows().len(),
            CLIPBOARD_CELLS + 1,
            "记录一行一条，末尾再加一行控制"
        );
        assert!(layout.is_clipboard(), "这一页该认得自己是剪贴板页");
        for row in &layout.rows()[..CLIPBOARD_CELLS] {
            assert_eq!(row.keys.len(), 1, "一行就一条记录");
            assert!(
                (KeyboardLayout::row_width(row, unit, gap) - width).abs() < 0.01,
                "每条记录都该铺满整宽（长文本才放得下）"
            );
        }

        let control = layout.rows().last().expect("该有控制行");
        assert_eq!(
            control.keys.len(),
            2,
            "返回 / 清空——翻页那两个 2026-09-21 撤了"
        );
        let control_width = KeyboardLayout::row_width(control, unit, gap);
        assert!(
            (width * 0.3..width * 0.6).contains(&control_width),
            "控制行该窄一点、居中，实际 {control_width}"
        );
    }

    /// 别的页不该被认成剪贴板页——那句话只写在剪贴板页中间。
    #[test]
    fn only_the_clipboard_page_says_it_is_one() {
        for panel in [Panel::Letters, Panel::Digits, Panel::Symbols, Panel::Tools] {
            assert!(!KeyboardLayout::of(panel).is_clipboard(), "{panel:?}");
        }
    }

    /// 工具页：三行图标格子（一行 5 格，与数字 / 符号页同一个单位宽）+ 一整行「返回」。
    #[test]
    fn the_tools_page_is_rows_of_icon_cells() {
        let layout = KeyboardLayout::tools();
        let (width, gap) = (360.0, 8.0);
        let unit = layout.unit_width(width, gap);

        assert_eq!(layout.rows().len(), 4, "跟别的页一样四行");
        for row in &layout.rows()[..3] {
            assert_eq!(row.keys.len(), TOOLS_PER_ROW, "图标格子一行 5 个");
            assert!(
                (KeyboardLayout::row_width(row, unit, gap) - width).abs() < 0.01,
                "这几行该铺满整宽"
            );
        }
        let back = layout.rows().last().expect("该有返回那一行");
        assert!(back.has_fill(), "「返回」撑满整行——这一行就它一个键");
    }

    #[test]
    fn letters_layout_is_26_letters_plus_nine_function_keys() {
        let layout = KeyboardLayout::letters();
        let total: usize = layout.rows().iter().map(|row| row.keys.len()).sum();
        // 26 字母 + Shift / 退格 / 中英 / 空格 / 逗号 / 句号 / 回车 / 123 / 符
        // （第 3、4 行共 9 个功能键）
        assert_eq!(total, 26 + 9);
        assert_eq!(layout.rows().len(), 4);
    }

    /// 撑满的键只出现在**键数比第 1 行少**的那两排，位置也固定。
    ///
    /// 「整排跟上面一样宽」这件事本身在这层看不出来（撑满键的宽度要等画的时候才知道），
    /// 由 `renderer::keyboard` 按像素盯。这条盯的是结构：哪一排、几个。
    #[test]
    fn only_the_short_rows_have_filling_keys() {
        let layout = KeyboardLayout::letters();
        let filling: Vec<(usize, usize)> = layout
            .rows()
            .iter()
            .enumerate()
            .filter(|(_, row)| row.has_fill())
            .map(|(index, row)| {
                let count = row
                    .keys
                    .iter()
                    .filter(|key| key.width == KeyWidth::Fill)
                    .count();
                (index, count)
            })
            .collect();

        assert_eq!(
            filling,
            vec![(2, 2), (3, 1)],
            "该只有第 3 行（⇧ 与 ⌫ 两个）与最下一排（空格一个）有撑满的键"
        );
    }

    /// 最下一排的键序：**逗号在空格左边、句号在右边，中 / 英在句号与回车之间**。
    #[test]
    fn the_letters_bottom_row_puts_the_comma_before_the_space() {
        let layout = KeyboardLayout::letters();
        let ids: Vec<KeyId> = layout.rows()[3].keys.iter().map(|key| key.id).collect();

        assert_eq!(
            ids,
            vec![
                KeyId::Panel(Panel::Symbols),
                KeyId::Panel(Panel::Digits),
                KeyId::Comma,
                KeyId::Space,
                KeyId::Period,
                KeyId::Mode,
                KeyId::Enter,
            ],
            "符 123 ， 空格 。 中/英 回车"
        );
    }

    /// **空格正落在这排的正中。**
    ///
    /// 这条踩过两次：中 / 英原在左边第二位（1.5 个单位），把空格往右顶了半格；
    /// 后来在空格右边补句号时，逗号落在左边、句号落在右边，等于只往左加了一格，偏得更狠
    /// （左边 4.5、右边 2.5，中心偏右整整 1 格）。现在两边各 3 个单位，偏 0。
    ///
    /// 判据是**空格中心的偏移**（左右差的一半），不是左右差本身——差 2 个单位才等于偏 1 格。
    #[test]
    fn the_space_bar_is_centred_in_its_row() {
        let layout = KeyboardLayout::letters();
        let row = &layout.rows()[3];
        let space = row
            .keys
            .iter()
            .position(|key| key.id == KeyId::Space)
            .expect("最下一排该有空格");

        let left: f32 = row.keys[..space].iter().map(Key::units).sum();
        let right: f32 = row.keys[space + 1..].iter().map(Key::units).sum();
        let offset = (left - right) / 2.0;

        assert_eq!(
            offset, 0.0,
            "空格左边 {left} 个单位、右边 {right}，中心偏了 {offset} 格"
        );
    }

    /// 26 个字母键**个个都带角标**，一个不多一个不少。
    ///
    /// 漏一个就是「这个键滑了没反应」，用户只会觉得是坏的；角标重复则是两个键滑出同一个字符，
    /// 也是错的。
    #[test]
    fn every_letter_key_carries_its_own_hint() {
        let layout = KeyboardLayout::letters();
        let hints: Vec<char> = layout
            .rows()
            .iter()
            .flat_map(|row| row.keys.iter())
            .filter(|key| matches!(key.id, KeyId::Letter(_)))
            .map(|key| key.hint.expect("字母键缺角标"))
            .collect();

        assert_eq!(hints.len(), 26, "角标数对不上：{hints:?}");
        let unique: std::collections::HashSet<char> = hints.iter().copied().collect();
        assert_eq!(unique.len(), 26, "有角标重复了：{hints:?}");
    }

    /// 有角标的只能是字母键——数字页、符号页那些键的键帽上写的就是它自己，
    /// 再挂个角标只会让人以为那个键能出两种字符。
    #[test]
    fn only_letter_keys_carry_hints() {
        for panel in [Panel::Digits, Panel::Symbols] {
            for row in KeyboardLayout::of(panel).rows() {
                for key in &row.keys {
                    assert!(key.hint.is_none(), "{panel:?} 上有键带了角标：{:?}", key.id);
                }
            }
        }
    }

    /// 每一页都得是四行——**键盘高度是定死的**，页与页行数不一样就会把上面的应用顶一下。
    #[test]
    fn every_panel_is_four_rows_of_five_units() {
        for panel in [Panel::Letters, Panel::Digits, Panel::Symbols] {
            let layout = KeyboardLayout::of(panel);
            assert_eq!(layout.rows().len(), 4, "{panel:?} 该是四行");
            if panel == Panel::Letters {
                continue;
            }
            for (index, row) in layout.rows().iter().enumerate() {
                assert_eq!(
                    row.weight(),
                    5.0,
                    "{panel:?} 第 {} 行不是 5 个单位宽",
                    index + 1
                );
            }
        }
    }

    /// 数字页照计算器那样排：1-2-3 在最上面一行，0 在最下，左边一竖条是运算符。
    #[test]
    fn digits_are_laid_out_like_a_calculator() {
        let layout = KeyboardLayout::digits();
        let texts: Vec<Vec<String>> = layout
            .rows()
            .iter()
            .map(|row| {
                row.keys
                    .iter()
                    .map(|key| match key.id {
                        KeyId::Literal(c) => c.to_string(),
                        KeyId::Panel(panel) => format!("{panel:?}"),
                        other => format!("{other:?}"),
                    })
                    .collect()
            })
            .collect();

        assert_eq!(texts[0][..3], ["+", "1", "2"], "第一行该是 + 与 1 2 3");
        assert_eq!(texts[0][4], "Backspace");
        assert_eq!(texts[1][..3], ["-", "4", "5"]);
        assert_eq!(texts[2][..3], ["*", "7", "8"]);
        assert_eq!(
            texts[3][..3],
            ["/", "Letters", "0"],
            "最后一行是 / 返回 0：回退该在「中」原来那个位置"
        );
    }

    /// 符号页该带的符号一个不少（键帽上是半角原字符，中文模式的全角由引擎转）。**没有 @**——那一格让给「回字母页」了。
    #[test]
    fn the_symbol_page_carries_the_symbols() {
        let layout = KeyboardLayout::symbols();
        let literals: Vec<char> = layout
            .rows()
            .iter()
            .flat_map(|row| row.keys.iter())
            .filter_map(|key| match key.id {
                KeyId::Literal(c) => Some(c),
                _ => None,
            })
            .collect();

        for expected in [
            '[', ']', '{', '}', '#', '%', '^', '&', '_', '=', '!', '?', '.',
        ] {
            assert!(literals.contains(&expected), "符号页少了 {expected}");
        }
        assert_eq!(literals.len(), 13, "符号页的符号数对不上：{literals:?}");
    }

    #[test]
    fn no_row_is_wider_than_the_keyboard() {
        let layout = KeyboardLayout::letters();
        let (width, gap) = (720.0, 6.0);
        let unit = layout.unit_width(width, gap);
        for row in layout.rows() {
            let row_width = KeyboardLayout::row_width(row, unit, gap);
            assert!(
                row_width <= width + 0.01,
                "行宽 {row_width} 超过了键盘宽 {width}"
            );
        }
    }

    #[test]
    fn the_widest_row_fills_the_keyboard() {
        let layout = KeyboardLayout::letters();
        let (width, gap) = (720.0, 6.0);
        let unit = layout.unit_width(width, gap);
        let widest = layout
            .rows()
            .iter()
            .map(|row| KeyboardLayout::row_width(row, unit, gap))
            .fold(0.0, f32::max);
        // 最宽那行应当正好铺满，否则说明定单位的公式和排布的公式对不上
        assert!(
            (widest - width).abs() < 0.01,
            "最宽行是 {widest}，应是 {width}"
        );
    }
}
