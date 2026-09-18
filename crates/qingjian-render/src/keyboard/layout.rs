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
    /// 字母页：26 键全键盘。
    ///
    /// 最下一行左边多了个 `123`（去数字页），空格的宽度是从它那儿让出来的，这一行还是 9 个单位宽。
    /// 不含 `'`（隔音符号）：字母排不下，它现在住在符号页。见 `docs/design/keyboard.md`。
    pub fn letters() -> Self {
        let row = |letters: &str| KeyRow {
            keys: letters
                .chars()
                .map(|c| Key::letter(c.to_ascii_uppercase()))
                .collect(),
        };

        Self {
            rows: vec![
                row("qwertyuiop"),
                row("asdfghjkl"),
                KeyRow {
                    keys: [
                        vec![Key::new(KeyId::Shift, 1.5)],
                        row("zxcvbnm").keys,
                        vec![Key::new(KeyId::Backspace, 1.5)],
                    ]
                    .concat(),
                },
                KeyRow {
                    keys: vec![
                        // 字母页**直接进得了数字页与符号页**，不必先绕一层。
                        // 这一行原先 9 个单位宽，插两个切页键之后把空格从 5 让到 3，
                        // **还是 9 个**——不然它会变成最宽的一行，把整块键盘的键都挤小
                        Key::new(KeyId::Panel(Panel::Symbols), 1.0),
                        Key::new(KeyId::Mode, 1.5),
                        Key::new(KeyId::Panel(Panel::Digits), 1.0),
                        Key::new(KeyId::Space, 3.0),
                        Key::new(KeyId::Comma, 1.0),
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
    use super::{KeyId, KeyboardLayout, Panel};

    #[test]
    fn letters_layout_is_26_letters_plus_eight_function_keys() {
        let layout = KeyboardLayout::letters();
        let total: usize = layout.rows().iter().map(|row| row.keys.len()).sum();
        // 26 字母 + Shift / 退格 / 中英 / 空格 / 逗号 / 回车 / 123 / 符（第 3、4 行共 8 个功能键）
        assert_eq!(total, 26 + 8);
        assert_eq!(layout.rows().len(), 4);
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
