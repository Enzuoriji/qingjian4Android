//! 展开面板（安卓）：候选铺成多行网格，**盖住键盘那一块**。
//!
//! 候选条是一条能横滚的带子，一屏看得见三到六格，想找第三屏那个词得一直滑。
//! 面板是它的补充：长按候选条弹出来，一屏铺几十个，**键盘让开、不压缩**
//! （2026-09-22 用户定的样子）。看完点一个上屏，点外面或者按返回就收起来。
//!
//! 与候选条共用三样东西，所以两边看着是一个东西的两种摆法：
//!
//! - **量宽**：`bar_cell_widths`（`renderer::bar` 里），一个词该多宽两边一个口径
//! - **画法**：`draw_bar_row`，云朵、居中、截断补省略号都在里面
//! - **行高**：`candidate_row_height`，同一个候选占的地方一样大
//!
//! 不一样的是摆法：候选条横着铺开、能横滚；面板折成多行、**竖着滚**。
//! 滚动的方向跟着手指走（照剪贴板列表那套），所以 [`CandidateGrid`] 量的是纵向的
//! 「一共多高、能滚多远」。
//!
//! **高度是会话给的**（= 键盘高度），不是按内容量出来的：面板要正好盖住键盘，
//! 矮一点会露出半截键盘、高一点会把候选条顶上去。所以这里没有 `preferred_size`，
//! 与候选条同一条路子。

mod grid;
mod rendered;

pub use grid::CandidateGrid;
pub use rendered::RenderedPanel;

use super::bar::Band;
use super::{Metrics, Rendered, Renderer};
use crate::canvas::Canvas;
use crate::error::RenderError;
use crate::frame::Row;
use crate::theme::Theme;

/// 面板要盖住的那块地方（点）。
///
/// 三个数凑一起才说得清「盖住键盘」这件事：宽是通栏的、高照键盘、底部还得跟键盘一样
/// 让开系统手势条。分着传会一路拖成八参数（clippy 会拦），也看不出它们是一件事。
#[derive(Debug, Clone, Copy)]
pub struct PanelArea {
    /// 宽（点）——通栏，就是输入视图那么宽。
    pub width: f32,

    /// 整块地方的高（点，**含** [`Self::bottom_inset`] 那一段）。
    ///
    /// 与键盘位图同高：`KeyboardTheme::height` 加让开的那一段（见
    /// [`Renderer::render_keyboard`]）。矮一点会露出半截键盘、高一点会把候选条顶上去，
    /// 所以这个数只能照键盘来，不能按内容量。
    pub height: f32,

    /// 底部让开系统手势条 / 导航栏多少（点）。
    ///
    /// **与键盘同一个数**：键盘的位图里那一段根本不画键（键只排到 `theme.height` 为止），
    /// 面板不裁的话最后一行会伸进去被导航栏压住——2026-09-23 在模拟器上就是这么露的馅。
    pub bottom_inset: f32,
}

impl Renderer {
    /// 画展开面板：候选折成多行铺满 `area` 那块地方。
    ///
    /// `scroll` 是纵向滚了多远（像素，内容坐标），会话按它把内容往上挪。
    /// `rows` 是**整份候选**（会话截到上限那一批），不是可见的那几格——
    /// 铺成网格之后哪几行看得见要等排完才知道，所以整份都得进来。
    ///
    /// 没有阴影：面板与屏幕边、候选条边齐平，四边不露在外面（与候选条同一条理由）。
    pub fn render_candidates_panel(
        &mut self,
        rows: &[Row],
        area: PanelArea,
        theme: &Theme,
        scale: f32,
        scroll: f32,
    ) -> Result<RenderedPanel, RenderError> {
        let m = Metrics { theme, scale };
        let content_width = (area.width * scale).round().max(1.0);
        let content_height = (area.height * scale).round().max(1.0);
        // 能铺东西的高度：底部那一段让给系统手势条（位图还是整块，只是底下空着）
        let usable = (content_height - m.px(area.bottom_inset)).max(0.0);
        let mut canvas = Canvas::new(content_width as u32, content_height as u32)?;
        canvas.fill_rect(
            0.0,
            0.0,
            content_width,
            content_height,
            theme.colors.background,
        );

        let grid = self.candidate_grid(rows, content_width, theme, scale);
        // 滚过头了就夹回来：候选从多变少时调用处那个位移是按老内容算的，
        // 不夹的话画出来是一片空白（比停在这一屏的末尾难受）
        let scroll = scroll.clamp(0.0, grid.max_scroll(usable));
        let row_height = grid.row_height();
        // 候选画在一张**只有可用高度**的图上，再整张贴回来：滚到一半时上下两行各露一截，
        // 伸出可用区的那部分得裁掉（底下那一段是留给系统手势条的）。
        // 画布自己不裁（`blend_pixel` 只挡位图边界），所以先画小图再贴——
        // 与键盘的剪贴板页同一个路数。
        let mut sheet = Canvas::new(content_width as u32, usable.round().max(1.0) as u32)?;
        for index in grid.visible(scroll, usable) {
            let (Some(cell), Some(candidate)) = (grid.cell(index), rows.get(index)) else {
                continue;
            };
            // 内容坐标减掉位移才是画布上的位置
            let top = cell.y - scroll;
            self.draw_bar_row(
                &mut sheet,
                &m,
                candidate,
                (cell.x, cell.width),
                Band {
                    top,
                    height: row_height,
                },
            );
        }
        canvas.blend_pixmap(0, 0, &sheet.into_pixmap());

        Ok(RenderedPanel {
            rendered: Rendered {
                pixmap: canvas.into_pixmap(),
                content_x: 0,
                content_y: 0,
                content_width: content_width as u32,
                content_height: content_height as u32,
                scale,
            },
            grid,
        })
    }
}
