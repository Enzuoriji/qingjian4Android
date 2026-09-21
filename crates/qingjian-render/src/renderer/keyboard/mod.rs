//! 画键盘：布局 + 状态 + 主题 → 位图，外加每个键的命中矩形。
//!
//! 与状态条（`renderer/status`）同一个套路：**画的同时把命中区一并返回**，壳只把原始触摸
//! 坐标传回来，由渲染器判断落在哪个键上。键盘是二维的，所以记的是矩形，不是状态条那种一条边界。

mod hit;
mod icon;
mod popup;
mod rendered;

use popup::IconPainter;

pub use hit::KeyHit;
pub use popup::Popup;
pub use rendered::RenderedKeyboard;

use super::{Rendered, Renderer};
use crate::canvas::Canvas;
use crate::error::RenderError;
use crate::keyboard::{
    CLIPBOARD_CELLS, InputMode, Key, KeyId, KeyWidth, KeyboardLayout, KeyboardState, Panel,
    ShiftState, TOOLS,
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
        let row_height = layout.row_height(rows_height, gap_y);

        let mut canvas = Canvas::new(pixels_wide, pixels_high)?;
        canvas.fill_rect(0.0, 0.0, content_width, content_height, theme.background);

        // 剪贴板页的**记录区**是一段能上下滚的窗口：卡片会滚过它的上下边界，画布又不裁，
        // 所以先画在一张只有记录区那么大的图上，再整张贴回来（与 `logo::draw_logo` 同一个路数）。
        // 别的页没有这一层，直接画在主画布上。
        let pitch = row_height + gap_y;
        let mut sheet = if layout.is_clipboard() {
            let height = (pitch * CLIPBOARD_CELLS as f32 - gap_y).round().max(1.0) as u32;
            Some(Canvas::new(pixels_wide, height)?)
        } else {
            None
        };
        // 整格的那部分已经由会话换掉了（喂进来的就是这一屏该画的几条），
        // 这里只让开不足一格的那点：滚动时卡片就是这么一格格挪上去的。
        // **不做除法**——「第几条起」是会话按同一套几何算的，两边各算一次会差出一格。
        let frac = if sheet.is_some() {
            state.clipboard_offset * scale
        } else {
            0.0
        };
        // 窗口几行画不画在小图上、命中区裁到多高——循环里要反复用，
        // 先取出来（`sheet` 待会儿会被可变借走）
        let sheet_height = sheet.as_ref().map_or(0.0, |sheet| sheet.height() as f32);

        let mut keys = Vec::new();
        let mut y = 0.0;
        for (index, row) in layout.rows().iter().enumerate() {
            // 窗口那几行画到小图上（顶边往上让开不足一格的那部分），其余行照旧
            let inside = sheet_height > 0.0 && index < CLIPBOARD_CELLS;
            let top = if inside { y - frac } else { y };
            let target = match sheet.as_mut() {
                Some(sheet) if inside => sheet,
                _ => &mut canvas,
            };
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
                    target,
                    key,
                    state,
                    theme,
                    scale,
                    (x, top, key_width, row_height),
                );
                // 滚出窗口的那部分不该还能点：命中区裁到窗口里，整个滚出去的就不报了
                let (hit_y, hit_height) = if inside {
                    let bottom = (top + row_height).min(sheet_height);
                    (top.max(0.0), bottom - top.max(0.0))
                } else {
                    (top, row_height)
                };
                if hit_height > 0.0 {
                    keys.push(KeyHit {
                        id: key.id,
                        x,
                        y: hit_y,
                        width: key_width,
                        height: hit_height,
                    });
                }
                x += key_width + gap_x;
            }
            y += pitch;
        }

        if let Some(sheet) = sheet {
            canvas.blend_pixmap(0, 0, &sheet.into_pixmap());
        }

        // 剪贴板空着时中间写一句：不写的话整块键盘上只剩底下那两个控制键，看着像坏了
        if layout.is_clipboard() && state.clipboard.is_empty() {
            self.draw_blank_clipboard(&mut canvas, theme, scale, content_width, rows_height);
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

    /// 剪贴板一条都没有时，键盘中间写一句话（[`BLANK_CLIPBOARD`]）。
    ///
    /// 用淡一档的颜色（`label_hint`）——它是句说明，不是键帽上的字。
    fn draw_blank_clipboard(
        &mut self,
        canvas: &mut Canvas,
        theme: &KeyboardTheme,
        scale: f32,
        width: f32,
        height: f32,
    ) {
        let style = TextStyle::new(
            theme.font.scaled(scale),
            theme.font.size,
            theme.label_hint,
            theme.text_gamma,
        );
        let size = self.measure(BLANK_CLIPBOARD, &style);
        self.draw_text(
            canvas,
            BLANK_CLIPBOARD,
            &style,
            (width - size.width) / 2.0,
            (height - size.height) / 2.0,
        );
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
        let text = label(key, state);
        // 剪贴板这一屏没那么多条 / 工具页这一格还没排工具：整个不画（连键帽都不画），
        // 免得空着一块白格子
        let sheet = matches!(key.id, KeyId::Clipboard(_));
        let sparse = matches!(key.id, KeyId::Clipboard(_) | KeyId::Tool(_));
        if sparse && text.is_empty() {
            return;
        }
        let pressed = state.pressed == Some(key.id);
        let mut cap = theme.key_color(key.style(), pressed);
        // Shift 锁定着换成强调色，一眼看出还开着
        if key.id == KeyId::Shift && state.shift == ShiftState::Locked {
            cap = theme.accent;
        }
        // 剪贴板那种格子是**一张卡片**：左右各缩一点，别铺到屏幕边上——一条通到两边
        // 看着像一整块面板，缩出两条缝才是一条条分开的（搜狗那些剪贴板条目就是这样）。
        // 命中区照旧是整行宽（`render_keyboard` 报的是没缩过的 slot），手指点哪都算数。
        let inset = if sheet { CARD_INSET * scale } else { 0.0 };
        let radius = (theme.radius * scale).min(width / 2.0).min(height / 2.0);
        canvas.fill_round_rect(x + inset, y, width - inset * 2.0, height, radius, cap);

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
            KeyId::Shift => icon::draw_shift(
                canvas,
                cx,
                main_cy,
                icon::size_on_key(height, scale),
                theme.label,
            ),
            KeyId::Backspace => icon::draw_backspace(
                canvas,
                cx,
                main_cy,
                icon::size_on_key(height, scale),
                theme.label,
            ),
            // 工具页的格子：**图标在上、名字在下**（搜狗那个面板就是这个样子），
            // 跟「一个大字居中」的键帽不是一回事，所以整个格子自己画
            KeyId::Tool(index) => {
                let label_style = TextStyle::new(
                    theme.hint_font.scaled(scale),
                    theme.hint_font.size,
                    theme.label,
                    theme.text_gamma,
                );
                let label_size = self.measure(&text, &label_style);
                let icon_size = height * TOOL_ICON_RATIO;
                // 图标、名字、上下三段的留白平分
                let gap = (height - icon_size - label_size.height) / 3.0;
                if let Some(draw) = tool_icon(index) {
                    draw(
                        canvas,
                        cx,
                        y + gap + icon_size / 2.0,
                        icon_size,
                        theme.label,
                    );
                }
                self.draw_text(
                    canvas,
                    &text,
                    &label_style,
                    cx - label_size.width / 2.0,
                    y + gap * 2.0 + icon_size,
                );
            }
            // 剪贴板那一格是**一段话**，不是键帽上的一个字：左边对齐、放不下截断补省略号
            // （复用候选条那套 `fit`，见 `renderer/bar`）。居中的话长文本两头都被切、认不出来。
            KeyId::Clipboard(_) => {
                let style = TextStyle::new(
                    theme.font.scaled(scale),
                    theme.font.size,
                    theme.label,
                    theme.text_gamma,
                );
                let pad = CELL_TEXT_PADDING * scale;
                // 文字从**卡片**左边起算，不是从格子的边起算（卡片自己已经缩进去一点了）
                let left = x + inset + pad;
                let text = self.fit(&text, &style, width - (inset + pad) * 2.0);
                let size = self.measure(&text, &style);
                self.draw_text(canvas, &text, &style, left, cy - size.height / 2.0);
            }
            _ => {
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

/// 剪贴板那种「一段话」的格子，字离左右边缘各留多少（点）。
///
/// 居中的键帽不留白也好看（字就一个），左边对齐的一长串贴边就难看了。
/// 从**卡片**的边算起，不是从格子的边（卡片自己还缩了 [`CARD_INSET`]）。
const CELL_TEXT_PADDING: f32 = 8.0;

/// 剪贴板那种卡片，左右各缩进来多少（点）。
///
/// 格子本身是铺满整宽的，卡片再缩这么一点——通到屏幕两边就成「一整块面板」了，
/// 缩出两条缝才看得出一条条分开。
const CARD_INSET: f32 = 4.0;

/// 剪贴板一条都没有时，键盘中间那行字。
const BLANK_CLIPBOARD: &str = "暂无剪贴板内容";

/// 工具页格子上那个图标占格子高度的多少。
///
/// 剩下的是名字与上下留白——格子是「宽比高长」的（一单位宽 × 一行高），
/// 图标给到 0.42 就够显眼了，再大就把名字挤出去。
const TOOL_ICON_RATIO: f32 = 0.42;

/// 工具页第 `index` 格画哪个图标（与 `TOOLS` 一一对应）。
///
/// 没排工具的格子是 `None`——那种格子上的字也是空的（见 `label`），整个不画。
fn tool_icon(index: usize) -> Option<IconPainter> {
    match index {
        0 => Some(icon::draw_clipboard),
        _ => None,
    }
}

/// 键帽上写什么字。图标键（Shift / 退格）由 [`Renderer::draw_key`] 提前分走，不会走到这里。
fn label(key: &Key, state: &KeyboardState) -> String {
    match key.id {
        // 剪贴板那一格写的是**那条文本**。喂进来的 `state.clipboard` 就是这一屏该画的
        // 那几条（会话按滚动量切好的），所以格号直接就是下标；滚到头、后面没那么多条时
        // 给空串，`draw_key` 见空就整个不画。
        KeyId::Clipboard(index) => state.clipboard.get(index).cloned().unwrap_or_default(),
        KeyId::Tool(index) => TOOLS.get(index).copied().unwrap_or_default().to_owned(),
        KeyId::ClipboardClear => "清空".to_owned(),
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
            // 工具页是标开的，页里没有再回工具页的键
            Panel::Tools | Panel::Clipboard => String::new(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{CARD_INSET, KeyHit, label};
    use crate::fonts::FontLibrary;
    use crate::keyboard::{InputMode, Key, KeyId, KeyboardLayout, KeyboardState, Panel};
    use crate::renderer::Renderer;
    use crate::theme::KeyboardTheme;

    /// 没有系统字体（CI 容器）就跳过——下面几个测试都要画字。
    fn renderer() -> Option<Renderer> {
        FontLibrary::system("zh-CN").ok().map(Renderer::new)
    }

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
        let mut renderer = renderer()?;
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
            // 空行（工具页留着以后排工具的那两行）没有键，也就没有「两头」可比
            if slice.is_empty() {
                continue;
            }
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

    /// 数字页、符号页、工具页每行都是 5 个单位、一样宽，这条守着别退化。
    ///
    /// **剪贴板页故意不在这条里**：它上面三行两格铺满、下面那行四个键窄一点居中
    /// （见 `layout.rs` 的 `the_clipboard_page_cells_fill_the_row`）。
    #[test]
    fn every_panel_has_rows_of_the_same_width() {
        for panel in [Panel::Digits, Panel::Symbols, Panel::Tools] {
            assert_rows_line_up(panel, None);
        }
    }

    /// 剪贴板那几格写的就是**喂进来的那几条**，格号即下标。
    ///
    /// 「从整份里的第几条起」是会话切好的（它才知道列表滚到哪儿了）——渲染器这边
    /// 收到的就是这一屏该画的几条。滚到头、后面没那么多条时给空串，
    /// `draw_key` 见空就整个不画（连卡片都不画）。
    #[test]
    fn the_clipboard_cells_show_the_entries_they_are_given() {
        let entries: Vec<String> = (0..5).map(|index| format!("第 {index} 条")).collect();
        let state = KeyboardState {
            clipboard: &entries,
            ..KeyboardState::default()
        };
        let cell = |index| Key::new(KeyId::Clipboard(index), 5.0);

        assert_eq!(label(&cell(0), &state), "第 0 条");
        assert_eq!(label(&cell(4), &state), "第 4 条");
    }

    /// 滚到最后一屏时后面那几格是空的——会话会给不足一屏的切片。
    #[test]
    fn a_short_clipboard_leaves_the_last_cells_blank() {
        let entries = ["第 0 条".to_owned(), "第 1 条".to_owned()];
        let state = KeyboardState {
            clipboard: &entries,
            ..KeyboardState::default()
        };
        assert_eq!(
            label(&Key::new(KeyId::Clipboard(1), 5.0), &state),
            "第 1 条"
        );
        assert_eq!(
            label(&Key::new(KeyId::Clipboard(2), 5.0), &state),
            "",
            "只给两条时第三格该是空的"
        );
    }

    /// 一条都没有时哪一格都是空的。
    #[test]
    fn an_empty_clipboard_draws_no_cells() {
        let state = KeyboardState::default();
        assert_eq!(
            label(&Key::new(KeyId::Clipboard(0), 5.0), &state),
            "",
            "一条都没有时第一格也是空的"
        );
    }

    /// 剪贴板那一条是**一张卡片**：比格子窄一点，左右各缩 [`CARD_INSET`]，不铺到屏幕两边。
    ///
    /// 用户 2026-09-21 要「仿搜狗的样式」——一条通到屏幕两边就成了「一整块面板」，
    /// 缩出两条缝才看得出一条条分开。命中区不受影响，还是整行宽（手指点哪都算数）。
    #[test]
    fn the_clipboard_entries_are_cards() {
        let Some(mut renderer) = renderer() else {
            return;
        };
        let entries = ["你好".to_owned(), "世界".to_owned()];
        let state = KeyboardState {
            clipboard: &entries,
            ..KeyboardState::default()
        };
        let scale = 2.0;
        let out = renderer
            .render_keyboard(
                &KeyboardLayout::clipboard(),
                &state,
                360.0,
                0.0,
                &KeyboardTheme::light(),
                scale,
            )
            .unwrap();

        let hit = out
            .keys
            .iter()
            .find(|key| key.id == KeyId::Clipboard(0))
            .expect("该有第一格");
        // 卡片是近白（252）、底色是浅灰（220）——沿这一格的中线扫一遍，取白的那一段
        // （字是黑的，只夹在中间，不影响两头）
        let pixmap = &out.rendered.pixmap;
        let line = (hit.y + hit.height / 2.0) as u32;
        let mut span = hit.x as u32..(hit.x + hit.width) as u32;
        let white = |x: u32| {
            pixmap
                .pixel(x, line)
                .is_some_and(|p| p.red() > 240 && p.green() > 240)
        };
        let left = span.clone().find(|&x| white(x)).expect("该扫到卡片");
        let right = span.rfind(|&x| white(x)).expect("该扫到卡片");

        let inset = (left as f32 - hit.x) / scale;
        assert!(
            (inset - CARD_INSET).abs() < 1.0,
            "卡片左边该缩进来 {CARD_INSET} 点，实际 {inset}"
        );
        let inset = (hit.x + hit.width - 1.0 - right as f32) / scale;
        assert!(
            (inset - CARD_INSET).abs() < 1.0,
            "卡片右边该缩进来 {CARD_INSET} 点，实际 {inset}"
        );
    }
}
