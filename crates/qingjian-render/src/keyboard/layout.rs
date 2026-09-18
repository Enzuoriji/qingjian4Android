//! 键盘布局：一行一行的按键。
//!
//! 几何按「单位宽」算，不写死坐标——每行按自己的总宽**居中**摆放，第 2 行（9 个键）自然
//! 得到半键错位，不用单独记缩进量。一个单位多宽由 [`KeyboardLayout::unit_width`] 取最挤的那一行定。

use super::key::{Key, KeyId};

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
    /// 26 键全键盘。
    ///
    /// 本轮不含 `123` / 符号面板（那两块还没做，摆个按下去没反应的键反而误导），
    /// 也不含 `'`（隔音符号，字母排不下，留给符号面板那一轮）。见 `docs/design/keyboard.md`。
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
                        Key::new(KeyId::Mode, 1.5),
                        Key::new(KeyId::Space, 5.0),
                        Key::new(KeyId::Comma, 1.0),
                        Key::new(KeyId::Enter, 1.5),
                    ],
                },
            ],
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
    use super::KeyboardLayout;

    #[test]
    fn letters_layout_is_26_letters_plus_eight_function_keys() {
        let layout = KeyboardLayout::letters();
        let total: usize = layout.rows().iter().map(|row| row.keys.len()).sum();
        // 26 字母 + Shift / 退格 / 中英 / 空格 / 逗号 / 回车 / …（第 3、4 行共 6 个功能键）
        assert_eq!(total, 26 + 6);
        assert_eq!(layout.rows().len(), 4);
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
