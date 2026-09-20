//! 候选条（安卓）：屏幕宽的一条，上排拼音、下排候选。
//!
//! 与候选窗（`renderer::{vertical,horizontal}`）的差别是结构级的：候选窗的宽高由内容量出来，
//! 候选条**宽度是屏幕宽**，高度由主题算死、不按内容量（没有 `preferred_size`）。
//!
//! 高度只在**「在不在组句」**这一个开关上变：组句当中是定值（候选从 0 个变 6 个不会动），
//! **没组句时整条收起来**（[`Renderer::bar_height`] 是 0），省下的高度还给应用。
//! 代价是敲第一个字母时上面的应用内容会被顶一下——明知故犯，见那个函数的注释。
//!
//! 拼音行直接复用候选窗那一套 [`Renderer::draw_top_line`]——分段、纠错删除线、光标都在里面，
//! 只有候选行是新写的。命中矩形与键盘一样随位图一并返回，壳只回传原始坐标。

mod hit;
mod id;
mod rendered;

pub use hit::BarHit;
pub use id::BarHitId;
pub use rendered::RenderedBar;

use super::{INDEX_GAP, Metrics, Rendered, Renderer};
use crate::canvas::Canvas;
use crate::error::RenderError;
use crate::frame::{Frame, Row};
use crate::text::TextStyle;
use crate::theme::Theme;

/// 清空拼音的记号。`×` 是乘号（U+00D7），不是字母 x。
const CLEAR: &str = "×";

/// 上一页 / 下一页。
const PAGE_PREV: &str = "‹";
const PAGE_NEXT: &str = "›";

/// 按钮左右各留的空白（点）——字本身很窄，不留白手指点不准。上下占满整排。
const BUTTON_PAD: f32 = 9.0;

/// 按钮之间的间距（点）。
const BUTTON_GAP: f32 = 4.0;

/// 词太长装不下时截断补的记号。
const ELLIPSIS: &str = "…";

/// 工具条的高度（点）。
///
/// 候选条底下那条常驻的按钮排——切键盘、设置齿轮那一类。**现在还没有，是 0**；
/// 将来加的时候改这里，**它不受组句与否影响**：没打字时它照样在。
const TOOLBAR_HEIGHT: f32 = 0.0;

impl Renderer {
    /// 候选条该占多高（点）。
    ///
    /// 分两段：
    /// - **工具条**（[`TOOLBAR_HEIGHT`]）常驻，将来放设置 / 工具按钮
    /// - **组字区**（拼音行 + 候选行）只在组句时占位；**不组句时整段收起来**，
    ///   省下的高度还给应用——键盘贴着应用下沿，一敲字母再顶出来
    ///
    /// 收起来这件事与「固定高度」那条原则**故意相反**：原先高度定死是为了不顶应用，
    /// 但空着一条几十点高的白带更难受，两害相权取其轻。代价是敲第一个字母时
    /// 上面的应用内容会被顶一下。
    pub fn bar_height(theme: &Theme, composing: bool) -> f32 {
        TOOLBAR_HEIGHT
            + if composing {
                Self::composing_height(theme)
            } else {
                0.0
            }
    }

    /// 组字区的高度（点）：拼音行 + 候选行 + 上下留白。
    fn composing_height(theme: &Theme) -> f32 {
        theme.padding * 2.0 + top_line_height(theme) + candidate_row_height(theme)
    }

    /// 画候选条，返回位图与每块可点区域。
    ///
    /// **高度为 0 的时候不要调这里**（没组句、也没有工具条时就是那样，0 高的位图建不出来）。
    /// 调用方看 [`Self::bar_height`] 先判一下——`Session::bar_surface` 就是这么做的。
    ///
    /// `width` 是内容宽度（点）——安卓传屏幕宽除以密度。没有阴影：候选条上下都与屏幕边、
    /// 键盘边齐平，四边不露在外面。
    pub fn render_bar(
        &mut self,
        frame: &Frame,
        width: f32,
        theme: &Theme,
        scale: f32,
    ) -> Result<RenderedBar, RenderError> {
        let m = Metrics { theme, scale };
        let content_width = (width * scale).round().max(1.0);
        let content_height = (Self::bar_height(theme, frame.preedit.is_some()) * scale)
            .round()
            .max(1.0);
        let mut canvas = Canvas::new(content_width as u32, content_height as u32)?;
        canvas.fill_rect(
            0.0,
            0.0,
            content_width,
            content_height,
            theme.colors.background,
        );

        let mut hits = Vec::new();
        let top = m.padding();
        let top_band = Band {
            top,
            height: m.px(top_line_height(theme)),
        };
        // 上排：拼音那一行整个复用候选窗的画法
        self.draw_top_line(&mut canvas, frame, &m, 0.0, top);
        self.draw_bar_buttons(&mut canvas, frame, &m, content_width, top_band, &mut hits);
        // 下排：候选
        let rows_band = Band {
            top: top_band.bottom(),
            height: m.px(candidate_row_height(theme)),
        };
        self.draw_bar_rows(&mut canvas, frame, &m, content_width, rows_band, &mut hits);

        Ok(RenderedBar {
            rendered: Rendered {
                pixmap: canvas.into_pixmap(),
                content_x: 0,
                content_y: 0,
                content_width: content_width as u32,
                content_height: content_height as u32,
                scale,
            },
            hits,
        })
    }

    /// 上排右端的清空与翻页：从右边缘往左摆。没有拼音就不画清空，只有一页就不画翻页。
    fn draw_bar_buttons(
        &mut self,
        canvas: &mut Canvas,
        frame: &Frame,
        m: &Metrics,
        right: f32,
        band: Band,
        hits: &mut Vec<BarHit>,
    ) {
        let mut x = right - m.padding();
        if frame.preedit.is_some() {
            x = self.draw_bar_piece(
                canvas,
                m,
                Piece::button(CLEAR, BarHitId::Clear),
                x,
                band,
                hits,
            );
        }
        let Some(footer) = frame.footer.as_deref() else {
            return;
        };
        x = self.draw_bar_piece(
            canvas,
            m,
            Piece::button(PAGE_NEXT, BarHitId::PageNext),
            x,
            band,
            hits,
        );
        // 页码本身不可点，夹在两个箭头中间
        x = self.draw_bar_piece(canvas, m, Piece::label(footer), x, band, hits);
        self.draw_bar_piece(
            canvas,
            m,
            Piece::button(PAGE_PREV, BarHitId::PagePrev),
            x,
            band,
            hits,
        );
    }

    /// 从 `right` 往左摆一块（按钮或页码），返回下一块的右边缘（已让开间距）。
    fn draw_bar_piece(
        &mut self,
        canvas: &mut Canvas,
        m: &Metrics,
        piece: Piece<'_>,
        right: f32,
        band: Band,
        hits: &mut Vec<BarHit>,
    ) -> f32 {
        let style = m.index_style();
        // 按钮左右留白，手指才点得准；页码不可点，不留
        let pad = if piece.hit.is_some() {
            m.px(BUTTON_PAD)
        } else {
            0.0
        };
        let size = self.measure(piece.text, &style);
        let width = size.width + pad * 2.0;
        let left = right - width;
        self.draw_text(
            canvas,
            piece.text,
            &style,
            left + pad,
            band.centre(size.height),
        );
        if let Some(id) = piece.hit {
            hits.push(BarHit {
                id,
                x: left,
                y: band.top,
                width,
                height: band.height,
            });
        }
        left - m.px(BUTTON_GAP)
    }

    /// 下排候选：可用宽度均分给每个候选，格内居中，格与格之间留一条缝。
    ///
    /// 均分而不是按自然宽度排，是为了让每格的**触摸面积一样大**——手指点的时候不用瞄。
    /// 每格两边各让出半条 `column_gap`：不留缝的话，三字词会正好顶满格子，
    /// 下一个候选的序号紧贴上一个词，一眼看过去像连成一个词。
    fn draw_bar_rows(
        &mut self,
        canvas: &mut Canvas,
        frame: &Frame,
        m: &Metrics,
        content_width: f32,
        band: Band,
        hits: &mut Vec<BarHit>,
    ) {
        if frame.rows.is_empty() {
            return;
        }
        let padding = m.padding();
        let slot = (content_width - padding * 2.0) / frame.rows.len() as f32;
        let gap = m.column_gap();
        let slot_width = (slot - gap).max(1.0);
        for (i, row) in frame.rows.iter().enumerate() {
            let left = padding + slot * i as f32 + gap / 2.0;
            if Some(i) == frame.highlighted {
                self.fill_highlight(canvas, m, left, band.top, slot_width, band.height);
            }
            self.draw_bar_row(canvas, m, row, (left, slot_width), band);
            // 命中区只覆盖画出来的这块，格子之间的缝不归任何候选（与键盘一致）
            hits.push(BarHit {
                id: BarHitId::Candidate(i),
                x: left,
                y: band.top,
                width: slot_width,
                height: band.height,
            });
        }
    }

    /// 画一个候选：序号 + 词（云端词前带云朵），整块在 `(x, 宽)` 的格子里居中。
    fn draw_bar_row(
        &mut self,
        canvas: &mut Canvas,
        m: &Metrics,
        candidate: &Row,
        slot: (f32, f32),
        band: Band,
    ) {
        let (x, width) = slot;
        let index_style = m.index_style();
        let text_style = m.text_style();
        let index = self.measure(&candidate.index, &index_style);
        let gap = m.px(INDEX_GAP);
        let cloud = if candidate.cloud {
            m.cloud_width()
        } else {
            0.0
        };
        // 格子宽度是硬约束，宁可少显示几个字也不能压到隔壁
        let text = self.fit(
            &candidate.text,
            &text_style,
            width - index.width - gap - cloud,
        );
        let text_size = self.measure(&text, &text_style);

        let text_height = m.px(m.theme.text_font.line_height);
        let top = band.centre(text_height);
        let mut left = x + (width - index.width - gap - cloud - text_size.width) / 2.0;
        self.draw_text(
            canvas,
            &candidate.index,
            &index_style,
            left,
            top + m.small_offset(text_height),
        );
        left += index.width + gap;
        if candidate.cloud {
            left += self.draw_cloud(canvas, m, left, top, text_height);
        }
        let color = if candidate.cloud {
            m.theme.colors.cloud
        } else {
            m.theme.colors.text
        };
        self.draw_text(canvas, &text, &m.style(m.theme.text_font, color), left, top);
    }

    /// 把 `text` 截到不超过 `max_width`，截过就补省略号；装得下原样返回。
    fn fit(&mut self, text: &str, style: &TextStyle, max_width: f32) -> String {
        if max_width <= 0.0 {
            return String::new();
        }
        if self.measure(text, style).width <= max_width {
            return text.to_owned();
        }
        let ellipsis = self.measure(ELLIPSIS, style).width;
        let mut kept = String::new();
        for c in text.chars() {
            let mut next = kept.clone();
            next.push(c);
            if self.measure(&next, style).width + ellipsis > max_width {
                break;
            }
            kept = next;
        }
        kept.push_str(ELLIPSIS);
        kept
    }
}

/// 候选条上一行的行框：顶边与高度（像素）。
///
/// 几个绘制助手共用同一条，省得「顶边 + 高度」这一对参数在每个签名里各传一遍。
#[derive(Debug, Clone, Copy)]
struct Band {
    /// 行框顶边。
    top: f32,

    /// 行框高度。
    height: f32,
}

impl Band {
    /// 行框底边。
    fn bottom(&self) -> f32 {
        self.top + self.height
    }

    /// 一块 `content_height` 高的内容在这个行框里垂直居中后的顶边。
    fn centre(&self, content_height: f32) -> f32 {
        self.top + (self.height - content_height) / 2.0
    }
}

/// 上排右端摆的一块：文字 + 命中目标。
#[derive(Debug, Clone, Copy)]
struct Piece<'a> {
    text: &'a str,

    /// `None` 表示不可点（页号码）。
    hit: Option<BarHitId>,
}

impl<'a> Piece<'a> {
    /// 可点的按钮。
    fn button(text: &'a str, id: BarHitId) -> Self {
        Self {
            text,
            hit: Some(id),
        }
    }

    /// 不可点的文字（页码）。
    fn label(text: &'a str) -> Self {
        Self { text, hit: None }
    }
}

/// 上排拼音行占的高度（点）。
fn top_line_height(theme: &Theme) -> f32 {
    theme.annotation_font.line_height + theme.row_padding * 2.0
}

/// 下排候选行占的高度（点）。
fn candidate_row_height(theme: &Theme) -> f32 {
    theme.text_font.line_height + theme.row_padding * 2.0
}

#[cfg(test)]
mod tests {
    use super::{BarHitId, ELLIPSIS, Metrics, Renderer};
    use crate::fonts::FontLibrary;
    use crate::frame::{Frame, Preedit, Row};
    use crate::theme::Theme;

    /// 验收用的屏幕宽（点），按一台常见手机的竖屏。
    const WIDTH: f32 = 360.0;

    /// 点 → 像素倍数。
    const SCALE: f32 = 2.0;

    /// 没有系统字体（CI 容器）就跳过——下面量的全是排版，没字体重不出来。
    fn renderer() -> Option<Renderer> {
        FontLibrary::system("zh-CN").ok().map(Renderer::new)
    }

    fn frame(rows: usize) -> Frame {
        Frame {
            preedit: Some(Preedit::plain("ni'hao", 6)),
            rows: (0..rows).map(|i| Row::plain(i, "你好")).collect(),
            highlighted: Some(0),
            footer: Some("1/6".to_owned()),
            sentence: None,
            status: None,
        }
    }

    fn candidates(frame: &super::RenderedBar) -> Vec<&super::BarHit> {
        frame
            .hits
            .iter()
            .filter(|hit| matches!(hit.id, BarHitId::Candidate(_)))
            .collect()
    }

    fn button(frame: &super::RenderedBar, id: BarHitId) -> &super::BarHit {
        frame
            .hits
            .iter()
            .find(|hit| hit.id == id)
            .unwrap_or_else(|| panic!("没找到 {id:?}"))
    }

    #[test]
    fn height_stays_the_same_whether_there_is_content_or_not() {
        let Some(mut renderer) = renderer() else {
            return;
        };
        let theme = Theme::light();
        // 有拼音但一个候选都没有（`ni'h` 这种还拼不成音节的）也是一个高度
        let bare = renderer
            .render_bar(&frame(0), WIDTH, &theme, SCALE)
            .unwrap();
        let full = renderer
            .render_bar(&frame(6), WIDTH, &theme, SCALE)
            .unwrap();
        assert_eq!(
            bare.rendered.content_height, full.rendered.content_height,
            "组句当中高度必须定死，否则候选一多一少就把应用顶一下"
        );
        assert_eq!(
            full.rendered.content_height,
            (Renderer::bar_height(&theme, true) * SCALE).round() as u32
        );
        assert!(
            candidates(&bare).is_empty(),
            "没候选时不该有候选格子（清空与翻页按钮还在，那是另一回事）"
        );
    }

    /// **没在组句时这一条整个收起来**——高度是 0，不是「矮一点」。
    ///
    /// 收起来省下的高度还给应用，键盘贴着应用下沿；一敲字母再顶出来。
    /// 与「组句当中高度定死」不矛盾：变的只是**在不在组句**这一个开关。
    #[test]
    fn the_bar_has_no_height_when_not_composing() {
        let theme = Theme::light();

        assert_eq!(
            Renderer::bar_height(&theme, false),
            0.0,
            "没组句时不该占任何高度"
        );
        assert!(Renderer::bar_height(&theme, true) > 0.0, "组句时该有高度");
    }

    #[test]
    fn candidate_slots_are_evenly_spaced_and_leave_a_gap() {
        let Some(mut renderer) = renderer() else {
            return;
        };
        let theme = Theme::light();
        let out = renderer
            .render_bar(&frame(6), WIDTH, &theme, SCALE)
            .unwrap();
        let slots = candidates(&out);
        assert_eq!(slots.len(), 6);
        let gap = theme.column_gap * SCALE;
        for pair in slots.windows(2) {
            assert!(pair[0].x < pair[1].x, "格子要按从左到右排");
            let space = pair[1].x - (pair[0].x + pair[0].width);
            assert!(
                (space - gap).abs() < 0.01,
                "格与格之间要正好留一条 {}，实际 {space}",
                gap
            );
        }
        // 每格一样宽：手指点的时候不用瞄
        let width = slots[0].width;
        assert!(slots.iter().all(|slot| (slot.width - width).abs() < 0.01));
        // 整排在对齐的左右边距里居中
        let padding = theme.padding * SCALE;
        let left = slots.first().unwrap().x;
        let right = slots.last().unwrap().x + slots.last().unwrap().width;
        assert!((left - padding - gap / 2.0).abs() < 0.01, "左边距");
        assert!(
            (right - (out.rendered.content_width as f32 - padding - gap / 2.0)).abs() < 0.01,
            "右边距"
        );
    }

    #[test]
    fn hit_finds_the_slot_under_the_point() {
        let Some(mut renderer) = renderer() else {
            return;
        };
        let out = renderer
            .render_bar(&frame(6), WIDTH, &Theme::light(), SCALE)
            .unwrap();
        let third = button(&out, BarHitId::Candidate(2));
        let inside = (third.x + third.width / 2.0, third.y + third.height / 2.0);
        assert_eq!(out.hit(inside.0, inside.1), Some(BarHitId::Candidate(2)));
        // 左边界算闭区间：自己这块的起点归自己
        assert_eq!(out.hit(third.x, inside.1), Some(BarHitId::Candidate(2)));
        // 右边界算开区间，正好落在右边那条缝上——缝不归任何候选
        assert_eq!(out.hit(third.x + third.width, inside.1), None);
        assert_eq!(out.hit(third.x - 0.5, inside.1), None);
        // 拼音行那一排不归任何候选
        assert_eq!(out.hit(inside.0, third.y - 1.0), None);
    }

    #[test]
    fn top_buttons_sit_side_by_side_at_the_right_edge() {
        let Some(mut renderer) = renderer() else {
            return;
        };
        let out = renderer
            .render_bar(&frame(6), WIDTH, &Theme::light(), SCALE)
            .unwrap();
        let clear = button(&out, BarHitId::Clear);
        let prev = button(&out, BarHitId::PagePrev);
        let next = button(&out, BarHitId::PageNext);
        assert!(prev.x < next.x, "上一页在左、下一页在右");
        assert!(next.x + next.width <= clear.x, "翻页不能压到清空");
        assert!(
            (clear.x + clear.width - (out.rendered.content_width as f32 - 8.0 * SCALE)).abs()
                < 0.01,
            "清空要贴住右边缘"
        );
        let center = |hit: &super::BarHit| (hit.x + hit.width / 2.0, hit.y + hit.height / 2.0);
        assert_eq!(
            out.hit(center(clear).0, center(clear).1),
            Some(BarHitId::Clear)
        );
        assert_eq!(
            out.hit(center(next).0, center(next).1),
            Some(BarHitId::PageNext)
        );
        assert_eq!(
            out.hit(center(prev).0, center(prev).1),
            Some(BarHitId::PagePrev)
        );
    }

    #[test]
    fn long_words_are_cut_short_with_an_ellipsis() {
        let Some(mut renderer) = renderer() else {
            return;
        };
        let theme = Theme::light();
        let m = Metrics {
            theme: &theme,
            scale: 1.0,
        };
        let style = m.text_style();
        let long = "你好你好你好你好你好";
        let fitted = renderer.fit(long, &style, 40.0);
        assert!(fitted.ends_with(ELLIPSIS));
        assert!(fitted.chars().count() < long.chars().count());
        assert!(renderer.measure(&fitted, &style).width <= 40.01);
        // 装得下就原样返回，不补省略号
        assert_eq!(renderer.fit("你好", &style, 1000.0), "你好");
    }
}
