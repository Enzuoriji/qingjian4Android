//! 按住键时弹出来的预览气泡：圆角块 + 阴影 + 一个放大的字。
//!
//! **单独出一张小位图，不画在键盘那张里**：气泡要弹到键盘**上方**去，而键盘位图就那么大，
//! 画在里面会被窗口裁掉。壳把它贴到一个浮动的小窗上（安卓侧是 `PopupWindow`），
//! 摆哪儿由 `Session` 按命中矩形算好告诉它——**壳只负责放到该放的地方，不参与画**，
//! 与候选条、键盘同一个规矩（见 `docs/design/rendering.md` 的显示面 / 控件面分类）。

use super::{Rendered, Renderer};
use crate::canvas::Canvas;
use crate::color::Color;
use crate::error::RenderError;
use crate::keyboard::{Key, KeyId, KeyboardState};
use crate::shadow::Shadow;
use crate::text::TextStyle;
use crate::theme::KeyboardTheme;

/// 气泡比键帽宽多少倍。
const WIDTH_RATIO: f32 = 1.3;

/// 气泡比键帽高多少倍。
///
/// 比宽略给多一点：气泡的用处是「手指挡住键帽时还能看清按的是哪个键」，
/// 竖着多出来的地方正是留给被挡住的那一块。
const HEIGHT_RATIO: f32 = 1.35;

/// 气泡的最小宽度（点）。
///
/// `，` `。` 这种窄键乘完倍率还是太窄，放不下那个放大的字。
const MIN_WIDTH: f32 = 36.0;

/// 气泡的最大宽度（点）。回车那种宽键乘完倍率会撑成一张大饼。
const MAX_WIDTH: f32 = 72.0;

/// 提示文字两侧各留的空白（点）。
const TEXT_PADDING: f32 = 10.0;

/// 提示文字的外宽上限（点）。比键帽宽是应该的——说不清就白提示了；
/// 但也不能宽到盖掉半块键盘。
const MAX_TEXT_WIDTH: f32 = 120.0;

/// 选项格子里字符两侧各留的空白（点）。
const CHOICE_PADDING: f32 = 8.0;

/// 一排选项的总宽上限（点）。三个格子再宽也不能盖掉半块键盘。
const MAX_CHOICES_WIDTH: f32 = 160.0;

/// 选项高亮垫底往格子里收多少（点）——不收就正好顶满，看着像格子粘在一起。
const CHOICE_INSET: f32 = 2.0;

/// 气泡里画什么。
pub enum Popup<'a> {
    /// 画这个键的样子——字母 / 字词画字，图标键（⇧ / ⌫）画图标。
    Key(&'a Key),

    /// 画一句提示。键上那个图标说不清「松手会怎样」这种事，得用话讲。
    Text(&'a str),

    /// **一排字符选项，`selected` 那个高亮**——长按字母键时弹的就是它。
    ///
    /// 三个格子等宽，往左滑选第一个、往右滑选最后一个（见 `docs/design/keyboard.md`）。
    Choices { items: [char; 3], selected: usize },
}

/// 画一个图标的函数签名：`(画布, 中心 x, 中心 y, 边长, 颜色)`，都是像素。
///
/// 键帽上那两个（⇧ / ⌫）与工具页那一格（剪贴板）是同一个签名——图标多大由调用方算好
/// （键帽按 [`super::icon::size_on_key`]，工具格子按自己的比例）。
pub(super) type IconPainter = fn(&mut Canvas, f32, f32, f32, Color);

/// 图标键要画哪个图标（画法与键帽上那个是同一份）。其余键没有图标，画字。
fn icon_of(id: KeyId) -> Option<IconPainter> {
    match id {
        KeyId::Shift => Some(super::icon::draw_shift),
        KeyId::Backspace => Some(super::icon::draw_backspace),
        _ => None,
    }
}

impl Renderer {
    /// 画一个键的预览气泡。`key_width` / `key_height` 是这个键的尺寸（点）。
    ///
    /// 返回的位图**四周带阴影留白**，`content_x` / `content_y` 才是气泡本体在里面的位置——
    /// 壳照着算摆放位置时要把这段留白减掉（照 `render_status` 的规矩）。
    pub fn render_key_popup(
        &mut self,
        content: Popup<'_>,
        state: &KeyboardState,
        key_width: f32,
        key_height: f32,
        theme: &KeyboardTheme,
        scale: f32,
    ) -> Result<Rendered, RenderError> {
        let shadow = Shadow::mac_panel();
        let content_height = key_height * HEIGHT_RATIO;
        let content_width = match &content {
            Popup::Key(_) => (key_width * WIDTH_RATIO).clamp(MIN_WIDTH, MAX_WIDTH),
            // 提示**按文字撑开**：说不清就白提示了。用键帽那个字号，别用气泡里那个放大的
            Popup::Text(text) => {
                let style = TextStyle::new(
                    theme.font.scaled(scale),
                    theme.font.size,
                    theme.label,
                    theme.text_gamma,
                );
                let size = self.measure(text, &style);
                (size.width / scale + TEXT_PADDING * 2.0).clamp(MIN_WIDTH, MAX_TEXT_WIDTH)
            }
            // 一排等宽的格子：按**最宽那个**算格宽，三个格子一样宽才像个滑动的轨道
            Popup::Choices { items, .. } => {
                // 与下面画的时候同一个字号（气泡里那个放大的）
                let style = TextStyle::new(
                    theme.popup_font.scaled(scale),
                    theme.popup_font.size,
                    theme.label,
                    theme.text_gamma,
                );
                let widest = items
                    .iter()
                    .map(|c| self.measure(&c.to_string(), &style).width)
                    .fold(0.0, f32::max);
                let cell = widest / scale + CHOICE_PADDING * 2.0;
                (cell * items.len() as f32).clamp(MIN_WIDTH, MAX_CHOICES_WIDTH)
            }
        };
        let margin = shadow.margin();
        let width = ((content_width + margin * 2.0) * scale).ceil();
        let height = ((content_height + margin * 2.0) * scale).ceil();
        let mut canvas = Canvas::new(width as u32, height as u32)?;

        // 气泡的圆角比键帽大一档——它是个浮起来的东西，圆一点才不像是键帽掉了出来
        let radius = theme.radius * 2.0 * scale;
        let (left, top) = (margin * scale, margin * scale);
        let (w, h) = (content_width * scale, content_height * scale);
        if let Some(content) = tiny_skia::Rect::from_xywh(left, top, w, h) {
            shadow.paint(&mut canvas, content, radius, scale);
        }
        canvas.fill_round_rect(left, top, w, h, radius, theme.popup);

        let (cx, cy) = (left + w / 2.0, top + h / 2.0);
        // 提示文字用键帽那个字号（小一档），键自己的字与选项都用气泡那个放大的
        let font = if matches!(content, Popup::Text(_)) {
            theme.font
        } else {
            theme.popup_font
        };
        let style = TextStyle::new(font.scaled(scale), font.size, theme.label, theme.text_gamma);

        if let Popup::Choices { items, selected } = &content {
            // 一排等宽的格子，选中的那个垫一块底色——手指滑到哪儿一眼看得出来
            let cell = w / items.len() as f32;
            let inset = CHOICE_INSET * scale;
            for (i, choice) in items.iter().enumerate() {
                let cell_left = left + cell * i as f32;
                if i == *selected {
                    canvas.fill_round_rect(
                        cell_left + inset,
                        top + inset,
                        cell - inset * 2.0,
                        h - inset * 2.0,
                        radius / 2.0,
                        theme.key_pressed,
                    );
                }
                let text = choice.to_string();
                let size = self.measure(&text, &style);
                self.draw_text(
                    &mut canvas,
                    &text,
                    &style,
                    cell_left + cell / 2.0 - size.width / 2.0,
                    cy - size.height / 2.0,
                );
            }
        } else {
            let (text, icon) = match &content {
                Popup::Text(text) => (text.to_string(), None),
                Popup::Key(key) => (super::label(key, state), icon_of(key.id)),
                Popup::Choices { .. } => unreachable!("上面那条分支已经处理过了"),
            };
            if let Some(draw) = icon {
                draw(
                    &mut canvas,
                    cx,
                    cy,
                    super::icon::size_on_key(h, scale),
                    theme.label,
                );
            } else {
                let size = self.measure(&text, &style);
                self.draw_text(
                    &mut canvas,
                    &text,
                    &style,
                    cx - size.width / 2.0,
                    cy - size.height / 2.0,
                );
            }
        }

        Ok(Rendered {
            pixmap: canvas.into_pixmap(),
            content_x: left.round() as u32,
            content_y: top.round() as u32,
            content_width: w.round() as u32,
            content_height: h.round() as u32,
            scale,
        })
    }
}
