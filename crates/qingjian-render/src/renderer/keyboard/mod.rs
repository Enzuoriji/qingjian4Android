//! 画键盘：布局 + 状态 + 主题 → 位图，外加每个键的命中矩形。
//!
//! 与状态条（`renderer/status`）同一个套路：**画的同时把命中区一并返回**，壳只把原始触摸
//! 坐标传回来，由渲染器判断落在哪个键上。键盘是二维的，所以记的是矩形，不是状态条那种一条边界。

mod hit;
mod icon;
mod rendered;

pub use hit::KeyHit;
pub use rendered::RenderedKeyboard;

use super::{Rendered, Renderer};
use crate::canvas::Canvas;
use crate::error::RenderError;
use crate::keyboard::{
    InputMode, Key, KeyId, KeyWidth, KeyboardLayout, KeyboardState, Panel, ShiftState,
};
use crate::text::TextStyle;
use crate::theme::KeyboardTheme;

impl Renderer {
    /// 画整个键盘，返回位图与每个键的命中矩形。
    ///
    /// `width` 是键盘内容宽度（点）——安卓传屏幕宽除以密度。总高是主题的 `height` 加
    /// `bottom_inset`：底部那条被系统手势条 / 导航栏占掉，键要往上摆，但底色照样铺到底。
    /// 没有阴影：键盘贴在屏幕底边，四边不露在外面。
    pub fn render_keyboard(
        &mut self,
        layout: &KeyboardLayout,
        state: &KeyboardState,
        width: f32,
        bottom_inset: f32,
        theme: &KeyboardTheme,
        scale: f32,
    ) -> Result<RenderedKeyboard, RenderError> {
        let content_width = width * scale;
        let content_height = (theme.height + bottom_inset) * scale;
        // 键只摆到底部那条之前，下面留给系统
        let rows_height = theme.height * scale;
        let gap_x = theme.gap_x * scale;
        let gap_y = theme.gap_y * scale;
        let pixels_wide = content_width.round().max(1.0) as u32;
        let pixels_high = content_height.round().max(1.0) as u32;

        let unit = layout.unit_width(content_width, gap_x);
        let row_count = layout.rows().len().max(1) as f32;
        let row_height = (rows_height - gap_y * (row_count - 1.0)) / row_count;

        let mut canvas = Canvas::new(pixels_wide, pixels_high)?;
        canvas.fill_rect(0.0, 0.0, content_width, content_height, theme.background);

        let mut keys = Vec::new();
        let mut y = 0.0;
        for row in layout.rows() {
            // 按单位宽算的那部分（不含撑满的键）。有撑满键的行**铺满整宽**，
            // 其余按自己的总宽居中——第 2 行（9 个键）由此自然得到半键错位。
            let fixed = KeyboardLayout::row_width(row, unit, gap_x);
            let mut x = if row.has_fill() {
                0.0
            } else {
                (content_width - fixed) / 2.0
            };
            // 撑满的键**平分**这一行剩下的（同一排里有几个就除以几；现在最多一个，
            // 但除以个数才对——不然两个会各自占满、叠在一起）
            let fills = row
                .keys
                .iter()
                .filter(|key| key.width == KeyWidth::Fill)
                .count();
            for key in &row.keys {
                let key_width = match key.width {
                    KeyWidth::Units(weight) => unit * weight,
                    KeyWidth::Fill => (content_width - fixed).max(0.0) / fills.max(1) as f32,
                };
                self.draw_key(
                    &mut canvas,
                    key,
                    state,
                    theme,
                    scale,
                    (x, y, key_width, row_height),
                );
                keys.push(KeyHit {
                    id: key.id,
                    x,
                    y,
                    width: key_width,
                    height: row_height,
                });
                x += key_width + gap_x;
            }
            y += row_height + gap_y;
        }

        Ok(RenderedKeyboard {
            rendered: Rendered {
                pixmap: canvas.into_pixmap(),
                content_x: 0,
                content_y: 0,
                content_width: pixels_wide,
                content_height: pixels_high,
                scale,
            },
            keys,
        })
    }

    /// 画一个键：圆角键帽，再加上文字或图标。`slot` 是 `(x, y, 宽, 高)`。
    fn draw_key(
        &mut self,
        canvas: &mut Canvas,
        key: &Key,
        state: &KeyboardState,
        theme: &KeyboardTheme,
        scale: f32,
        slot: (f32, f32, f32, f32),
    ) {
        let (x, y, width, height) = slot;
        let pressed = state.pressed == Some(key.id);
        let mut cap = theme.key_color(key.style(), pressed);
        // Shift 锁定着换成强调色，一眼看出还开着
        if key.id == KeyId::Shift && state.shift == ShiftState::Locked {
            cap = theme.accent;
        }
        let radius = (theme.radius * scale).min(width / 2.0).min(height / 2.0);
        canvas.fill_round_rect(x, y, width, height, radius, cap);

        let (cx, cy) = (x + width / 2.0, y + height / 2.0);

        // 角标先画，它在键帽上方偏上那一条
        if let Some(hint) = key.hint {
            let text = hint.to_string();
            let style = TextStyle::new(
                theme.hint_font.scaled(scale),
                theme.hint_font.size,
                theme.label_hint,
                theme.text_gamma,
            );
            let size = self.measure(&text, &style);
            self.draw_text(
                canvas,
                &text,
                &style,
                cx - size.width / 2.0,
                y + height * HINT_CENTER_Y - size.height / 2.0,
            );
        }

        // 有角标的键，主字往下让开那一条；没角标的键照旧居中，跟以前一样
        let main_cy = if key.hint.is_some() {
            cy + height * MAIN_SHIFT
        } else {
            cy
        };

        match key.id {
            KeyId::Shift => icon::draw_shift(canvas, cx, main_cy, height, scale, theme.label),
            KeyId::Backspace => {
                icon::draw_backspace(canvas, cx, main_cy, height, scale, theme.label)
            }
            _ => {
                let text = label(key, state);
                let style = TextStyle::new(
                    theme.font.scaled(scale),
                    theme.font.size,
                    theme.label,
                    theme.text_gamma,
                );
                let size = self.measure(&text, &style);
                self.draw_text(
                    canvas,
                    &text,
                    &style,
                    cx - size.width / 2.0,
                    main_cy - size.height / 2.0,
                );
            }
        }
    }
}

/// 角标中心落在键帽高度（从顶边算）的这个比例处。
///
/// 与 [`MAIN_SHIFT`] 是一对，照实机截图量的：角标压在键帽上沿、主字落在中线下一点。
const HINT_CENTER_Y: f32 = 0.22;

/// 有角标时主字往下挪的比例——不挪会和角标叠在一起。
const MAIN_SHIFT: f32 = 0.10;

/// 键帽上写什么字。图标键（Shift / 退格）由 [`Renderer::draw_key`] 提前分走，不会走到这里。
fn label(key: &Key, state: &KeyboardState) -> String {
    match key.id {
        KeyId::Letter(c) => {
            if state.shift.is_upper() {
                c.to_uppercase().to_string()
            } else {
                c.to_lowercase().to_string()
            }
        }
        KeyId::Mode => match state.mode {
            InputMode::Chinese => "中".to_owned(),
            InputMode::English => "英".to_owned(),
        },
        // 这两个跟着模式走：中文画全角、英文画半角。画死成全角的话，
        // 切到英文之后键帽上写着「。」打出来的却是 `.`，那是骗人
        KeyId::Comma => match state.mode {
            InputMode::Chinese => "，".to_owned(),
            InputMode::English => ",".to_owned(),
        },
        KeyId::Period => match state.mode {
            InputMode::Chinese => "。".to_owned(),
            InputMode::English => ".".to_owned(),
        },
        KeyId::Enter => "回车".to_owned(),
        KeyId::Space => String::new(),
        KeyId::Shift | KeyId::Backspace => String::new(),
        // 半角原字符照画：中文模式下它会变成全角，画死成全角在英文模式下就骗人了
        KeyId::Literal(c) => c.to_string(),
        // 写的是**要去哪一页**
        KeyId::Panel(panel) => match panel {
            Panel::Letters => "返回".to_owned(),
            Panel::Digits => "123".to_owned(),
            Panel::Symbols => "符".to_owned(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{KeyHit, label};
    use crate::fonts::FontLibrary;
    use crate::keyboard::{InputMode, Key, KeyId, KeyboardLayout, KeyboardState, Panel};
    use crate::renderer::Renderer;
    use crate::theme::KeyboardTheme;

    /// 逗号与句号**跟着中 / 英走**。
    ///
    /// 画死成全角的话，切到英文之后键帽上写着「。」、打出来却是 `.`——键帽骗人。
    /// 别的键（`123`、`符`、`回车`）是动作键，不随模式变。
    #[test]
    fn the_punctuation_keys_follow_the_mode() {
        let state = |mode| KeyboardState {
            mode,
            ..KeyboardState::default()
        };

        for (id, chinese, english) in [(KeyId::Comma, "，", ","), (KeyId::Period, "。", ".")] {
            let key = Key::new(id, 1.0);
            assert_eq!(
                label(&key, &state(InputMode::Chinese)),
                chinese,
                "{id:?} 中文"
            );
            assert_eq!(
                label(&key, &state(InputMode::English)),
                english,
                "{id:?} 英文"
            );
        }
    }

    /// 键上图标（⇧ / ⌫）的边长**占键高的比例**，量的是画出来的深色像素。
    fn icon_ratio(scale: f32) -> Option<(f32, f32)> {
        let library = FontLibrary::system("zh-CN").ok()?;
        let mut renderer = Renderer::new(library);
        let layout = KeyboardLayout::letters();
        let out = renderer
            .render_keyboard(
                &layout,
                &KeyboardState::default(),
                360.0,
                0.0,
                &KeyboardTheme::light(),
                scale,
            )
            .ok()?;

        let pixmap = &out.rendered.pixmap;
        let dark = |hit: &KeyHit| {
            let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0u32, 0u32);
            for y in hit.y as u32..(hit.y + hit.height) as u32 {
                for x in hit.x as u32..(hit.x + hit.width) as u32 {
                    let Some(p) = pixmap.pixel(x, y) else {
                        continue;
                    };
                    // 键帽是浅灰底、图标是近黑，取深的那撮
                    if p.red() < 120 && p.green() < 120 && p.blue() < 130 {
                        x0 = x0.min(x);
                        y0 = y0.min(y);
                        x1 = x1.max(x);
                        y1 = y1.max(y);
                    }
                }
            }
            (x1 - x0 + 1) as f32 / hit.height
        };

        let find = |id: KeyId| {
            let hit = out.keys.iter().find(|key| key.id == id)?;
            Some(dark(hit))
        };
        Some((find(KeyId::Shift)?, find(KeyId::Backspace)?))
    }

    /// **图标要跟着屏幕密度一起放大**——这条踩过：上限原先按像素写死，
    /// 而键高是像素值、随密度涨，于是密度越高的屏幕图标相对越小。
    /// 真机上「退格 / 上档图标偏小」就是这么来的，模拟器（密度 2）上却看着正常。
    #[test]
    fn the_icons_scale_with_density() {
        let Some((low_shift, low_back)) = icon_ratio(2.0) else {
            return;
        };
        let Some((high_shift, high_back)) = icon_ratio(3.0) else {
            return;
        };

        for (name, low, high) in [("⇧", low_shift, high_shift), ("⌫", low_back, high_back)] {
            assert!(
                (low - high).abs() < 0.02,
                "{name} 图标占键高的比例该与密度无关：密度 2 是 {low:.2}、密度 3 是 {high:.2}"
            );
            assert!(high > 0.25, "{name} 图标太小了：占键高 {high:.2}");
        }
    }

    /// 某一页每一排的左右边缘（像素），按行顺序。
    ///
    /// 命中矩形在 `keys` 里是**按行顺序**推的，所以按各行的键数切开就行，不用再按 y 分。
    fn row_edges(panel: Panel) -> Option<Vec<(f32, f32)>> {
        // 没有系统字体的环境（CI 容器）跳过
        let library = FontLibrary::system("zh-CN").ok()?;
        let mut renderer = Renderer::new(library);
        let layout = KeyboardLayout::of(panel);
        let out = renderer
            .render_keyboard(
                &layout,
                &KeyboardState::default(),
                360.0,
                0.0,
                &KeyboardTheme::light(),
                2.0,
            )
            .ok()?;

        let mut edges = Vec::new();
        let mut at = 0;
        for row in layout.rows() {
            let slice = &out.keys[at..at + row.keys.len()];
            at += row.keys.len();
            let left = slice.iter().map(|key| key.x).fold(f32::MAX, f32::min);
            let right = slice
                .iter()
                .map(|key| key.x + key.width)
                .fold(f32::MIN, f32::max);
            edges.push((left, right));
        }
        Some(edges)
    }

    /// 某一页几乎所有排的两头都该一样齐。
    ///
    /// `skip` 是允许不齐的那一排——字母页第 2 行（`asdfghjkl`）窄半键是**有意的错位**，
    /// 照实体键盘的排法。
    fn assert_rows_line_up(panel: Panel, skip: Option<usize>) {
        let Some(edges) = row_edges(panel) else {
            return;
        };
        let (left, right) = edges[0];
        for (index, (l, r)) in edges.iter().enumerate() {
            if Some(index) == skip {
                continue;
            }
            assert!(
                (l - left).abs() < 0.01 && (r - right).abs() < 0.01,
                "{panel:?} 第 {} 排是 ({l}, {r})，第 1 排是 ({left}, {right})，两头没对齐",
                index + 1
            );
        }
    }

    /// 字母页除了第 2 行（有意的半键错位），每一排的两头都要跟第 1 行**严丝合缝**。
    ///
    /// 第 3 行 9 个键（8 条缝）、最下一排 7 个（6 条缝），第 1 行有 10 个（9 条缝）——
    /// 全按固定单位宽排下来这两排都会窄一条、两头各缩进去一点。
    /// 靠 ⇧ / ⌫ / 空格这几个「撑满」的键吃掉多出来的那一段，才对齐。
    /// 这块来回错过四次，**别再凭眼睛看**。
    #[test]
    fn the_letters_rows_line_up() {
        assert_rows_line_up(Panel::Letters, Some(1));
    }

    /// 数字页、符号页每行都是 5 个键、一样宽，这条守着别退化。
    #[test]
    fn every_panel_has_rows_of_the_same_width() {
        for panel in [Panel::Digits, Panel::Symbols] {
            assert_rows_line_up(panel, None);
        }
    }
}
