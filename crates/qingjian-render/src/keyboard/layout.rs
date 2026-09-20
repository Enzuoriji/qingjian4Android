//! 键盘布局：一行一行的按键。
//!
//! 几何按「单位宽」算，不写死坐标——每行按自己的总宽**居中**摆放，第 2 行（9 个键）自然
//! 得到半键错位，不用单独记缩进量。一个单位多宽由 [`KeyboardLayout::unit_width`] 取最挤的那一行定。

use super::key::{Key, KeyId};
use super::panel::Panel;

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
    /// 这一行按键的总权重（不含缝隙）。
    pub fn weight(&self) -> f32 {
        self.keys.iter().map(|key| key.weight).sum()
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
    /// 最下一行左边多了个 `123`（去数字页），空格的宽度是从它那儿让出来的，这一行还是 9 个单位宽。
    /// 隔音符号 `'` 没有独立键位，现在挂在 `Z` 的角标上——它原先住符号页，符号页改版后没了着落。
    pub fn letters() -> Self {
        let [qwerty, home, bottom] = LETTER_ROWS.map(|(letters, hints)| letter_row(letters, hints));

        Self {
            rows: vec![
                qwerty,
                home,
                KeyRow {
                    keys: [
                        vec![Key::new(KeyId::Shift, 1.5)],
                        bottom.keys,
                        vec![Key::new(KeyId::Backspace, 1.5)],
                    ]
                    .concat(),
                },
                KeyRow {
                    keys: vec![
                        // 字母页**直接进得了数字页与符号页**，不必先绕一层
                        Key::new(KeyId::Panel(Panel::Symbols), 1.0),
                        Key::new(KeyId::Panel(Panel::Digits), 1.0),
                        // 逗号在空格**左边**、句号在右边，中 / 英再往右——**空格两边的键
                        // 各 3 个单位**，空格正好落在这一排的正中（见下面那条注释）
                        Key::new(KeyId::Comma, 1.0),
                        Key::new(KeyId::Space, 4.0),
                        Key::new(KeyId::Period, 1.0),
                        Key::new(KeyId::Mode, 1.0),
                        Key::new(KeyId::Enter, 1.0),
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

    /// 某一页的布局。
    pub fn of(panel: Panel) -> Self {
        match panel {
            Panel::Letters => Self::letters(),
            Panel::Digits => Self::digits(),
            Panel::Symbols => Self::symbols(),
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
            .filter(|row| !row.keys.is_empty())
            .map(|row| (width - gap * (row.keys.len() - 1) as f32) / row.weight())
            .fold(f32::MAX, f32::min)
    }

    /// 一行的总宽（点）。
    pub fn row_width(row: &KeyRow, unit: f32, gap: f32) -> f32 {
        unit * row.weight() + gap * (row.keys.len().saturating_sub(1)) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::{KeyId, KeyRow, KeyboardLayout, Panel};

    #[test]
    fn letters_layout_is_26_letters_plus_nine_function_keys() {
        let layout = KeyboardLayout::letters();
        let total: usize = layout.rows().iter().map(|row| row.keys.len()).sum();
        // 26 字母 + Shift / 退格 / 中英 / 空格 / 逗号 / 句号 / 回车 / 123 / 符
        // （第 3、4 行共 9 个功能键）
        assert_eq!(total, 26 + 9);
        assert_eq!(layout.rows().len(), 4);
    }

    /// 最下一排要跟第 1、3 行**一样宽**（10 个单位），左右才齐平。
    ///
    /// 原先只有 9 个（逗号在空格右边、没有句号），整排比上下两行各缩进去半个键，
    /// 一眼就看得出是歪的。
    #[test]
    fn the_letters_bottom_row_lines_up_with_the_rows_above() {
        let layout = KeyboardLayout::letters();
        let rows = layout.rows();
        let widest = rows.iter().map(KeyRow::weight).fold(0.0, f32::max);

        assert_eq!(widest, 10.0, "最宽的行该是 10 个单位");
        assert_eq!(
            rows[3].weight(),
            widest,
            "最下一排该跟第 1、3 行一样宽，不然整排是缩进去的"
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

        let left: f32 = row.keys[..space].iter().map(|key| key.weight).sum();
        let right: f32 = row.keys[space + 1..].iter().map(|key| key.weight).sum();
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
