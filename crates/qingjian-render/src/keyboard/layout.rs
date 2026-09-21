//! 键盘布局：一行一行的按键。
//!
//! 几何按「单位宽」算，不写死坐标——每行按自己的总宽**居中**摆放，第 2 行（9 个键）自然
//! 得到半键错位，不用单独记缩进量。一个单位多宽由 [`KeyboardLayout::unit_width`] 取最挤的那一行定。

use super::key::{Key, KeyId, KeyWidth};
use super::panel::Panel;

/// 一行的宽度，按单位算。**各页都是 5**：数字 / 符号页是五列，工具与剪贴板页也按这个排——
/// 单位宽取最挤的那一行，所以各页的行宽一致、格子边缘对得齐。
const ROW_UNITS: f32 = 5.0;

/// 剪贴板一屏几条（三行两格）。
pub const CLIPBOARD_CELLS: usize = 6;

/// 工具页上那几行的名字，顺序就是 [`KeyboardLayout::tools`] 的行序。
///
/// 现在只有剪贴板；「震动程度」「设置」这些以后往下排（页里留了空行）。
pub const TOOLS: [&str; 1] = ["剪贴板"];

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

    /// 工具页：一页一个工具，**整行宽**（一行 5 个单位，与数字 / 符号页同一个单位宽）。
    ///
    /// 空行是留着以后排工具的（震动、设置那些），没有键就不画。
    pub fn tools() -> Self {
        Self {
            rows: vec![
                KeyRow {
                    keys: vec![Key::new(KeyId::Tool(0), ROW_UNITS)],
                },
                KeyRow { keys: Vec::new() },
                KeyRow { keys: Vec::new() },
                KeyRow {
                    keys: vec![Key::new(KeyId::Panel(Panel::Letters), ROW_UNITS)],
                },
            ],
        }
    }

    /// 剪贴板页：三行两格的记录 + 一行控制。
    ///
    /// 记录格 2.5 个单位、控制行四个 1 个单位的键——都是 5 个单位一行，
    /// 所以**六格的左右边与别的页对得齐**（单位宽取最挤的那一行，这里是记录行）。
    pub fn clipboard() -> Self {
        let cell = |index: usize| Key::new(KeyId::Clipboard(index), ROW_UNITS / 2.0);
        Self {
            rows: vec![
                KeyRow {
                    keys: vec![cell(0), cell(1)],
                },
                KeyRow {
                    keys: vec![cell(2), cell(3)],
                },
                KeyRow {
                    keys: vec![cell(4), cell(5)],
                },
                KeyRow {
                    keys: vec![
                        Key::new(KeyId::Panel(Panel::Letters), 1.0),
                        Key::new(KeyId::ClipboardPage(-1), 1.0),
                        Key::new(KeyId::ClipboardPage(1), 1.0),
                        Key::new(KeyId::ClipboardClear, 1.0),
                    ],
                },
            ],
        }
    }

    /// 某一页的布局。
    pub fn of(panel: Panel) -> Self {
        match panel {
            Panel::Letters => Self::letters(),
            Panel::Digits => Self::digits(),
            Panel::Symbols => Self::symbols(),
            Panel::Tools => Self::tools(),
            Panel::Clipboard => Self::clipboard(),
        }
    }

    pub fn rows(&self) -> &[KeyRow] {
        &self.rows
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
    use super::{Key, KeyId, KeyWidth, KeyboardLayout, Panel};

    /// 剪贴板页：六格铺满整宽，控制行四个键窄一点居中。
    ///
    /// 单位宽取**最挤的那一行**，这里是最上面那三行记录格（两个键、5 个单位）——
    /// 所以六格的左右边与别的页对得齐，而下面那行四个键是居中的一组。
    #[test]
    fn the_clipboard_page_cells_fill_the_row() {
        let layout = KeyboardLayout::clipboard();
        let (width, gap) = (360.0, 8.0);
        let unit = layout.unit_width(width, gap);

        let cells = &layout.rows()[0];
        assert_eq!(cells.keys.len(), 2, "一屏两格");
        assert!(
            (KeyboardLayout::row_width(cells, unit, gap) - width).abs() < 0.01,
            "记录那两格该铺满整宽"
        );

        let control = layout.rows().last().expect("该有控制行");
        assert_eq!(control.keys.len(), 4, "返回 / 上一屏 / 下一屏 / 清空");
        let control_width = KeyboardLayout::row_width(control, unit, gap);
        assert!(
            (width * 0.7..width).contains(&control_width),
            "控制行该窄一点、居中，实际 {control_width}"
        );
    }

    /// 工具页：每一行整宽（一行 5 个单位，与数字 / 符号页同一个单位宽）。
    #[test]
    fn the_tools_page_rows_fill_the_width() {
        let layout = KeyboardLayout::tools();
        let (width, gap) = (360.0, 8.0);
        let unit = layout.unit_width(width, gap);

        assert_eq!(
            layout.rows().len(),
            4,
            "跟别的页一样四行，空行留着以后排工具"
        );
        for row in layout.rows().iter().filter(|row| !row.keys.is_empty()) {
            assert!(
                (KeyboardLayout::row_width(row, unit, gap) - width).abs() < 0.01,
                "工具页每行都该铺满整宽"
            );
        }
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
