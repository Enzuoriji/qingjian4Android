//! 候选条（安卓）：屏幕宽的一条，上排拼音、下排候选。
//!
//! 与候选窗（`renderer::{vertical,horizontal}`）的差别是结构级的：候选窗的宽高由内容量出来，
//! 候选条**宽度是屏幕宽**，高度由主题算死、不按内容量（没有 `preferred_size`）。
//!
//! 高度只在**「在不在组句」**这一个开关上变：组句当中是定值（候选从 0 个变 6 个不会动），
//! **没组句时收成细细一条**（[`IDLE_HEIGHT`]，里面就一个标），省下的高度还给应用。
//! 代价是敲第一个字母时上面的应用内容会被顶一下——明知故犯，见 [`Renderer::bar_height`] 的注释。
//!
//! 拼音行直接复用候选窗那一套 [`Renderer::draw_top_line`]——分段、纠错删除线、光标都在里面，
//! 只有候选行是新写的。命中矩形与键盘一样随位图一并返回，壳只回传原始坐标。

mod hit;
mod id;
mod rendered;
mod strip;

pub use hit::BarHit;
pub use id::BarHitId;
pub use rendered::RenderedBar;
pub use strip::BarStrip;

use super::{Metrics, Rendered, Renderer};
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

/// 每个候选格的**底线宽度**（点）。
///
/// 格宽改成按内容分之后，一个单字候选的自然宽度只有二十来点——没有这条底线就成了一根针，
/// 点都点不着。安卓自己建议的最小可点区域也是 48dp。
const MIN_CELL_WIDTH: f32 = 48.0;

/// 词在自己格子里两边各留的空白（点）。
///
/// 算「这个候选自然需要多宽」时加进去：一点都不留的话，词会正好顶满格子，
/// 和隔壁挨在一起看成一串。
const CELL_PADDING: f32 = 4.0;

/// 词太长装不下时截断补的记号。
const ELLIPSIS: &str = "…";

/// 那个标（四片竹简）画多高（点）。**量的是竹简本身**，不含 svg 里的留白——
/// 这条细的才 [`IDLE_HEIGHT`] 点，标占 24 点、上下各留 6 点。
const LOGO_HEIGHT: f32 = 24.0;

/// 标离左边留多少（点）——贴着边不好看，也别留太多（用户要「靠左」）。
const LOGO_LEFT: f32 = 5.0;

/// 标左右各留的空白（点）——图标本身就窄，不留白手指点不准。
const LOGO_PAD: f32 = 7.0;

/// 没组句时那一条该多高（点）：标上下各留一点，够手指点着。
///
/// 这条细的与候选条**共用同一块地方**（2026-09-21 用户拍板）：不打字时只有它，
/// 一打字候选条整个接管、标让开——打字的竖向空间一点没被它占掉。
///
/// 30 → 36（同一天用户提的「条子宽点，给标和下面 q 键留些距离」）：标本身 24 点不变，
/// 上下留白从 3 点变 6 点——标不再贴着自己的边，跟键盘第一行也隔开了。
/// 这与组句时的 66 点（拼音行 + 候选行 + 留白）仍差着一大截，那条原则没动。
const IDLE_HEIGHT: f32 = 36.0;

impl Renderer {
    /// 候选条该占多高（点）。**同一块地方，两态共用**：
    ///
    /// - **组句当中**：拼音行 + 候选行，与以前一模一样（标这时不画）
    /// - **没组句**：只剩一条细的（[`IDLE_HEIGHT`]），里面一个标
    ///
    /// 「没组句就收起来」那条原则没变（空着一条白带更难受），只是这条细的换成了个有用的按钮：
    /// 剪贴板、以后的设置都挂在这个标上。代价是没打字时应用少那么一条，
    /// 而**打字时的竖向空间一分没多**。
    pub fn bar_height(theme: &Theme, composing: bool) -> f32 {
        if composing {
            Self::composing_height(theme)
        } else {
            IDLE_HEIGHT
        }
    }

    /// 组字区的高度（点）：拼音行 + 候选行 + 上下留白。
    fn composing_height(theme: &Theme) -> f32 {
        theme.padding * 2.0 + top_line_height(theme) + candidate_row_height(theme)
    }

    /// 画候选条，返回位图与每块可点区域。**同一块地方两种画法**（见 [`Self::bar_height`]）：
    /// 组句当中是拼音行 + 候选行，没组句时只有左边一个标与旁边的页码。
    ///
    /// `width` 是内容宽度（点）——安卓传屏幕宽除以密度。没有阴影：候选条上下都与屏幕边、
    /// 键盘边齐平，四边不露在外面。
    pub fn render_bar(
        &mut self,
        frame: &Frame,
        width: f32,
        theme: &Theme,
        scale: f32,
        scroll: f32,
    ) -> Result<RenderedBar, RenderError> {
        let m = Metrics { theme, scale };
        let content_width = (width * scale).round().max(1.0);
        let composing = frame.preedit.is_some();
        let content_height = (Self::bar_height(theme, composing) * scale)
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
        if composing {
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
            self.draw_bar_rows(&mut canvas, frame, &m, rows_band, &mut hits, scroll);
        } else {
            self.draw_bar_tools(&mut canvas, frame, &m, &mut hits);
        }

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

    /// 没组句时那一条：左边是**青简那个标**（开 / 收工具页），右边跟着页码。
    ///
    /// 标走 [`crate::logo::draw_logo`]——**画路径不画字形**（那个字画不出形，见模块注释）。
    /// 命中区比标大一圈，手指点得着。
    ///
    /// 页码是给剪贴板翻页用的（[`Frame::footer`]）：候选那一套翻页在组句时才在，
    /// 而剪贴板页恰恰是没组句的时候——页码挪到这条上来才看得见。
    fn draw_bar_tools(
        &mut self,
        canvas: &mut Canvas,
        frame: &Frame,
        m: &Metrics,
        hits: &mut Vec<BarHit>,
    ) {
        let band = Band {
            top: 0.0,
            height: m.px(IDLE_HEIGHT),
        };
        let height = m.px(LOGO_HEIGHT);
        let pad = m.px(LOGO_PAD);
        let left = m.px(LOGO_LEFT);
        let width = crate::logo::draw_logo(canvas, left, band.centre(height), height);
        // 命中区从条子左边缘起、比标大一圈：标本身窄，按它算手指点不准
        hits.push(BarHit {
            id: BarHitId::Tools,
            x: 0.0,
            y: band.top,
            width: left + width + pad * 2.0,
            height: band.height,
        });

        let Some(footer) = frame.footer.as_deref() else {
            return;
        };
        let style = m.index_style();
        let text = self.measure(footer, &style);
        let x = left + width + pad * 2.0;
        self.draw_text(canvas, footer, &style, x, band.centre(text.height));
    }

    /// 上排右端的清空与翻页：从右边缘往左摆。没有拼音就不画清空，**带子一屏装得下就不画翻页**
    /// （那是会话定的：没有第二屏可去就不报页码，这里跟着没有页码就不画那两个箭头）。
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

    /// 下排候选：**一条能横着滚的带子**。
    ///
    /// 每格宽度**按内容定**（低于 [`MIN_CELL_WIDTH`] 才抬到那个底线，触摸目标不能太小），
    /// 所以多长的词都装得下、一格都不会被截。一格一格从左往右铺，起点是
    /// `左边距 − scroll`——`scroll` 就是这条带子被拖出去多远，**跟手平移**。
    /// 画到带子外面自然被位图裁掉（画布就这么宽），命中矩形照铺，抬手时会落在正确的格上。
    ///
    /// 早先每格等宽（触摸面积一样大），但格宽被「一页几个」除死：360 点宽的屏上一页 5 个，
    /// 一格 60.8 点，而四个汉字要 64 点——于是**逢四字词必截**（用户报的「超过三个字就省略」）。
    /// 参考项目 fcitx5-android 的候选条用 `FlexboxLayoutManager`，条目按自然宽度排，
    /// 也是这个路子（它不滚，满 `maxSpanCount` 就收，多的进展开面板）。
    fn draw_bar_rows(
        &mut self,
        canvas: &mut Canvas,
        frame: &Frame,
        m: &Metrics,
        band: Band,
        hits: &mut Vec<BarHit>,
        scroll: f32,
    ) {
        if frame.rows.is_empty() {
            return;
        }
        let gap = m.column_gap();
        let widths = self.bar_cell_widths(&frame.rows, m);
        let mut left = m.padding() - scroll;
        for (i, (row, width)) in frame.rows.iter().zip(&widths).enumerate() {
            if Some(i) == frame.highlighted {
                self.fill_highlight(canvas, m, left, band.top, *width, band.height);
            }
            self.draw_bar_row(canvas, m, row, (left, *width), band);
            // 命中区跟着这一格一起走；画到位图外面的那几格照样报，反正手指落不到那儿
            hits.push(BarHit {
                id: BarHitId::Candidate(i),
                x: left,
                y: band.top,
                width: *width,
                height: band.height,
            });
            left += width + gap;
        }
    }

    /// 下排候选每格多宽（点）：**这个词自然需要多宽就给多宽**，低于底线才抬到
    /// [`MIN_CELL_WIDTH`]（单字候选的自然宽度只有二十来点，没这条底线就成了点不着的针）。
    ///
    /// 不再「把一行铺满」：带子是能滚的，铺满就没有滚的余地了。
    fn bar_cell_widths(&mut self, rows: &[Row], m: &Metrics) -> Vec<f32> {
        let floor = m.px(MIN_CELL_WIDTH);
        rows.iter()
            .map(|row| {
                let text = self.measure(&row.text, &m.text_style()).width;
                let cloud = if row.cloud { m.cloud_width() } else { 0.0 };
                let natural = text + cloud + m.px(CELL_PADDING) * 2.0;
                natural.max(floor).max(1.0)
            })
            .collect()
    }

    /// 画一个候选：词（云端词前带云朵），整块在 `(x, 宽)` 的格子里居中。
    ///
    /// **不画序号**（2026-09-20）：候选条是手指点的，选哪一格靠位置不靠数字——
    /// 序号是实体键盘那套（`⇧ + 数字`）留下的，手指够不着，白占宽度。
    /// 一格约 68 点宽，序号连间距就吃掉十来点，去掉相当于**每个词多出半个字**，
    /// 三字词被截成「你…」的那条线也就跟着往后挪了。
    /// 序号仍在 [`Row::index`] 里（桌面候选窗要用），只是这里不画。
    fn draw_bar_row(
        &mut self,
        canvas: &mut Canvas,
        m: &Metrics,
        candidate: &Row,
        slot: (f32, f32),
        band: Band,
    ) {
        let (x, width) = slot;
        let text_style = m.text_style();
        let cloud = if candidate.cloud {
            m.cloud_width()
        } else {
            0.0
        };
        // 格子宽度是硬约束，宁可少显示几个字也不能压到隔壁
        let text = self.fit(&candidate.text, &text_style, width - cloud);
        let text_size = self.measure(&text, &text_style);

        let text_height = m.px(m.theme.text_font.line_height);
        let top = band.centre(text_height);
        let mut left = x + (width - cloud - text_size.width) / 2.0;
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
    ///
    /// 剪贴板那几格也用这条（那边是键盘，同属渲染器内部）。
    pub(super) fn fit(&mut self, text: &str, style: &TextStyle, max_width: f32) -> String {
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
    use super::{BarHitId, BarStrip, ELLIPSIS, MIN_CELL_WIDTH, Metrics, Renderer};
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
            .render_bar(&frame(0), WIDTH, &theme, SCALE, 0.0)
            .unwrap();
        let full = renderer
            .render_bar(&frame(6), WIDTH, &theme, SCALE, 0.0)
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

    /// **没在组句时这一条收成细细的一条**（里面就一个标），不是「整个没有」。
    ///
    /// 2026-09-21 之前是收到 0；现在那条细的换成个有用的按钮（剪贴板 / 设置那个标），
    /// 而**一打字就被候选条整个接管**——打字的竖向空间一点没多占（见 [`Renderer::bar_height`]）。
    #[test]
    fn the_bar_shrinks_to_one_thin_strip_when_not_composing() {
        let theme = Theme::light();
        let idle = Renderer::bar_height(&theme, false);
        let composing = Renderer::bar_height(&theme, true);

        assert!(idle > 0.0, "没组句时也该留一条细的（标要地方）");
        // 六成是个宽松的界：这条细的只装一个标，够不着组句时那个高度就行。
        // （2026-09-21 从 30 抬到 36，正好越过「一半」，所以这里不写 idle * 2 < composing）
        assert!(
            idle < composing * 0.6,
            "那条细的该比组句时矮一截：{idle} vs {composing}"
        );
    }

    /// 没组句时那一条上画的是标，**不是**拼音行与候选行。
    #[test]
    fn the_idle_strip_is_just_the_gear() {
        let Some(mut renderer) = renderer() else {
            return;
        };
        let theme = Theme::light();
        let bare = Frame {
            preedit: None,
            rows: Vec::new(),
            highlighted: None,
            footer: None,
            sentence: None,
            status: None,
        };
        let out = renderer
            .render_bar(&bare, WIDTH, &theme, SCALE, 0.0)
            .unwrap();

        assert_eq!(
            out.hits.iter().map(|hit| hit.id).collect::<Vec<_>>(),
            vec![BarHitId::Tools],
            "没组句时那块地方只有标可点"
        );
        // 标贴在左边，命中区够手指点
        let gear = button(&out, BarHitId::Tools);
        assert!(gear.x < WIDTH, "标该在左边");
        assert!(
            gear.width >= 24.0 * SCALE,
            "命中区太窄手指点不准：{}",
            gear.width
        );
    }

    /// 下排候选：从左边距起，一格一格往右铺，格与格之间留一条缝。
    ///
    /// **不再断言「铺满一行」**——2026-09-21 起这是一条**能滚的带子**，宽度是内容定的，
    /// 铺满就没有滚的余地了（原先等宽时确实铺满，那是为了触摸面积一样大）。
    /// 带子跟手不平移由 `scrolling_shifts_the_strip` 盯着。
    #[test]
    fn candidate_slots_start_at_the_padding_and_leave_a_gap() {
        let Some(mut renderer) = renderer() else {
            return;
        };
        let theme = Theme::light();
        let out = renderer
            .render_bar(&frame(6), WIDTH, &theme, SCALE, 0.0)
            .unwrap();
        let slots = candidates(&out);
        assert_eq!(slots.len(), 6);

        let padding = theme.padding * SCALE;
        let gap = theme.column_gap * SCALE;
        assert!(
            (slots.first().unwrap().x - padding).abs() < 0.01,
            "没滚动时第一格该从左边距起，实际 {}",
            slots.first().unwrap().x
        );
        for pair in slots.windows(2) {
            assert!(pair[0].x < pair[1].x, "格子要按从左到右排");
            let space = pair[1].x - (pair[0].x + pair[0].width);
            assert!(
                (space - gap).abs() < 0.01,
                "格与格之间要正好留一条 {gap}，实际 {space}"
            );
        }
    }

    /// **给多少 scroll，整条带子就左移多少**——「跟手滚」就是这么来的。
    #[test]
    fn scrolling_shifts_the_strip() {
        let Some(mut renderer) = renderer() else {
            return;
        };
        let theme = Theme::light();
        let scroll = 40.0;
        let rest = renderer
            .render_bar(&frame(6), WIDTH, &theme, SCALE, 0.0)
            .unwrap();
        let scrolled = renderer
            .render_bar(&frame(6), WIDTH, &theme, SCALE, scroll)
            .unwrap();

        let (rest, scrolled) = (candidates(&rest), candidates(&scrolled));
        assert_eq!(rest.len(), scrolled.len());
        for (a, b) in rest.iter().zip(&scrolled) {
            assert!(
                (a.x - b.x - scroll).abs() < 0.01,
                "每格都该整体左移 {scroll}：{} → {}",
                a.x,
                b.x
            );
            assert!((a.width - b.width).abs() < 0.01, "滚动不该改宽度");
        }
    }

    /// **四个汉字的词不该被截断**——用户报的「超过三个字就省略」。
    ///
    /// 格宽原先等分：360 点宽、一页 5 个，一格 60.8 点，而四个汉字要 64 点，**逢四字必截**。
    /// 现在按内容分——短词让出来的宽度补给长词。
    #[test]
    fn a_four_character_candidate_is_not_cut_short() {
        let Some(mut renderer) = renderer() else {
            return;
        };
        let theme = Theme::light();
        let m = Metrics {
            theme: &theme,
            scale: SCALE,
        };
        let style = m.text_style();

        // 长短混着来：四字词夹在单字、两字之间
        let mixed = Frame {
            preedit: Some(Preedit::plain("ni'hao", 6)),
            rows: vec![
                Row::plain(0, "你"),
                Row::plain(1, "你好"),
                Row::plain(2, "你好你好"),
                Row::plain(3, "你"),
                Row::plain(4, "你好"),
            ],
            highlighted: Some(0),
            footer: None,
            sentence: None,
            status: None,
        };
        let out = renderer
            .render_bar(&mixed, WIDTH, &theme, SCALE, 0.0)
            .unwrap();
        let slots = candidates(&out);

        let long = "你好你好";
        let needs = renderer.measure(long, &style).width;
        assert_eq!(
            renderer.fit(long, &style, slots[2].width),
            long,
            "四字词那格 {} 点该装得下（词本身要 {needs} 点）",
            slots[2].width
        );
        assert!(
            slots[2].width > slots[0].width,
            "长词那格该比单字那格宽：{} vs {}",
            slots[2].width,
            slots[0].width
        );
    }

    /// 每格都不低于底线——单字候选的自然宽度只有二十来点，没这条底线就成了点不着的针。
    #[test]
    fn every_slot_keeps_a_reachable_minimum() {
        let Some(mut renderer) = renderer() else {
            return;
        };
        let theme = Theme::light();
        let all_short = Frame {
            preedit: Some(Preedit::plain("ni", 2)),
            rows: (0..5).map(|i| Row::plain(i, "你")).collect(),
            highlighted: Some(0),
            footer: None,
            sentence: None,
            status: None,
        };
        let out = renderer
            .render_bar(&all_short, WIDTH, &theme, SCALE, 0.0)
            .unwrap();

        let floor = MIN_CELL_WIDTH * SCALE;
        for hit in candidates(&out) {
            assert!(
                hit.width >= floor - 0.01,
                "每格都不该低于底线 {floor}，实际 {}",
                hit.width
            );
        }
    }

    #[test]
    fn hit_finds_the_slot_under_the_point() {
        let Some(mut renderer) = renderer() else {
            return;
        };
        let out = renderer
            .render_bar(&frame(6), WIDTH, &Theme::light(), SCALE, 0.0)
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
            .render_bar(&frame(6), WIDTH, &Theme::light(), SCALE, 0.0)
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

    /// 三格宽 100 / 50 / 200、左边距 10、格间距 4：整条 378 宽，各格从左数 10、114、168 起。
    fn strip() -> BarStrip {
        BarStrip::new(&[100.0, 50.0, 200.0], 10.0, 4.0)
    }

    /// 一屏装得下就全画；装不下时画到装不下的前一格为止。
    #[test]
    fn the_strip_shows_the_cells_that_fit() {
        assert_eq!(
            strip().slice(0.0, 400.0),
            0..3,
            "378 宽的带子配 400 的屏，全看得见"
        );
        assert_eq!(strip().slice(0.0, 150.0), 0..2, "150 宽的屏装不下第三格");
    }

    /// 滚到哪儿画哪儿：第一格整个滚出去之后就不画它了。
    #[test]
    fn scrolling_leaves_the_cells_behind() {
        assert_eq!(strip().slice(160.0, 150.0), 1..3, "滚过第一格就不画它");
    }

    /// 一屏装得下就没得滚，也就没有「第几屏」这回事。
    #[test]
    fn a_strip_that_fits_on_one_screen_cannot_scroll() {
        assert_eq!(strip().max_scroll(400.0), 0.0);
        assert_eq!(strip().screens(400.0), 1);
    }

    /// 高亮认的是**整格**：滚到半格上时，第一个左边没被切掉的格才是它。
    #[test]
    fn the_highlighted_cell_is_the_first_one_that_is_not_cut() {
        // 三格的左边是 10、114、168
        assert_eq!(strip().first_whole(0.0), 0, "从头看起时是第一格");
        assert_eq!(strip().first_whole(50.0), 1, "第一格只露半边了，该认第二格");
        assert_eq!(
            strip().first_whole(114.0),
            1,
            "正好滚到第二格的左边，还是它"
        );
        assert_eq!(strip().first_whole(115.0), 2);
    }

    /// 喂给渲染的位移是「第一格相对视口左缘」：让它贴着左缘时，位移正好是那个左边距。
    #[test]
    fn the_render_offset_puts_the_first_cell_at_the_edge() {
        assert_eq!(strip().local_scroll(0, 0.0), 0.0, "从头看起时不用挪");
        assert_eq!(strip().local_scroll(1, 114.0), 10.0, "第二格贴左缘");
    }

    /// 页码的分子要能走到分母：一路 `›` 翻到底，正好停在「第 screens 屏」。
    #[test]
    fn paging_forward_stops_on_the_last_page() {
        // 每格 500 + 间距 10：八格一共 4090 宽，配 1000 的屏不止一屏
        let strip = BarStrip::new(&[500.0; 8], 10.0, 10.0);
        let viewport = 1000.0;
        let screens = strip.screens(viewport);
        assert!(screens > 1, "4090 宽的带子配 1000 的屏该不止一屏");

        let mut scroll = 0.0;
        for _ in 0..screens * 2 {
            let next = strip.screen_scroll(strip.screen(scroll, viewport) + 1, viewport);
            if next == scroll {
                break;
            }
            scroll = next;
        }
        assert_eq!(scroll, strip.max_scroll(viewport), "翻到底该停在最远处");
        assert_eq!(
            strip.screen(scroll, viewport),
            screens,
            "停下来时该正好是最后一屏——不然页码会停在 5/6 那种数上"
        );
    }
}
