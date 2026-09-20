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
use crate::keyboard::{InputMode, Key, KeyId, KeyboardLayout, KeyboardState, Panel, ShiftState};
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
            // 每行按自己的总宽居中，第 2 行自然得到半键错位
            let mut x = (content_width - KeyboardLayout::row_width(row, unit, gap_x)) / 2.0;
            for key in &row.keys {
                let key_width = unit * key.weight;
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
            KeyId::Shift => icon::draw_shift(canvas, cx, main_cy, height, theme.label),
            KeyId::Backspace => icon::draw_backspace(canvas, cx, main_cy, height, theme.label),
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
        // 这两个画全角：字母页底下最常用的标点，一眼认得出来。符号页那些符号仍画半角原字符
        KeyId::Comma => "，".to_owned(),
        KeyId::Period => "。".to_owned(),
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
