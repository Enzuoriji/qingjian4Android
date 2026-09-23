//! 展开面板：候选折成多行之后，每格在哪、一共多高、滚到哪儿了。
//!
//! 与候选条是**同一套量宽**（`Renderer::bar_cell_widths`）：一个词该多宽，两边口径一致，
//! 换个地方看不该变样。差别只在摆法——候选条横着铺成一条能横滚的带子，面板**折成多行**、
//! 能上下滚。
//!
//! 格宽只算一份、由渲染器算（[`Renderer::candidate_grid`]）：会话不自己再算一遍，
//! 否则画出来的位置与算出来的滚动范围会各走各的（与候选条同一条理由）。

use std::ops::Range;

use crate::frame::Row;
use crate::renderer::{Metrics, Renderer};
use crate::theme::Theme;

/// 一格在内容区里的位置（像素）。
///
/// 坐标是**内容坐标**：`y` 是「整块内容里的位置」，不随滚动变——画的时候才减掉位移。
/// 这样滚一下不必把每格重算一遍，命中也能直接拿当前位移去比。
#[derive(Debug, Clone, Copy)]
pub struct Cell {
    /// 左边缘。
    pub x: f32,

    /// 上边缘。
    pub y: f32,

    /// 宽度（已夹到不超过一行）。
    pub width: f32,
}

/// 整份候选折成多行之后的位置。
#[derive(Debug, Clone, Default)]
pub struct CandidateGrid {
    cells: Vec<Cell>,

    /// 一行多高（像素）。
    row_height: f32,

    /// 内容总高（像素，含上下留白）。
    total: f32,
}

impl CandidateGrid {
    /// 按每格的宽度从左往右摆，一行摆不下就换行。
    ///
    /// `content_width` 是面板宽（像素），`gap` 是格与格之间那道缝，`pad` 是四周留白。
    fn new(
        widths: &[f32],
        content_width: f32,
        row_height: f32,
        row_gap: f32,
        pad: f32,
        gap: f32,
    ) -> Self {
        // 装得下的宽度：一格比整行还宽时夹到这儿，不然它会把这一行撑出屏幕、
        // 后面那些格全跑到看不见的地方（画的时候再由 `fit` 截断补省略号）
        let avail = (content_width - pad * 2.0).max(1.0);
        let mut cells = Vec::with_capacity(widths.len());
        let mut x = pad;
        let mut y = pad;
        for &natural in widths {
            let width = natural.min(avail);
            // 换行只在**不是行首**时判：行首那格刚夹过，一定放得下，
            // 再判一次会把它单独顶到下一行去
            if x > pad && x + width > content_width - pad {
                x = pad;
                y += row_height + row_gap;
            }
            cells.push(Cell { x, y, width });
            x += width + gap;
        }
        // 走完一轮 `y` 停在最后一行的顶边，加上行高与下留白才是整块多高
        let total = if cells.is_empty() {
            0.0
        } else {
            y + row_height + pad
        };
        Self {
            cells,
            row_height,
            total,
        }
    }

    /// 一共几格。
    pub fn len(&self) -> usize {
        self.cells.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    /// 这一格在哪儿。
    pub(super) fn cell(&self, index: usize) -> Option<Cell> {
        self.cells.get(index).copied()
    }

    /// 一行多高（像素）。
    pub(super) fn row_height(&self) -> f32 {
        self.row_height
    }

    /// 内容总高（像素，含上下留白）。
    pub fn total_height(&self) -> f32 {
        self.total
    }

    /// 视口 `viewport` 高时最远能滚到哪儿。内容比一屏矮就是 0（没法滚）。
    pub fn max_scroll(&self, viewport: f32) -> f32 {
        (self.total - viewport).max(0.0)
    }

    /// 视口落在 `[scroll, scroll + viewport)` 时该画哪几格（左闭右开）。
    ///
    /// 两边各算上**只露出一半**的那些——画出去会被位图裁掉（`Canvas::blend_pixel` 有边界检查），
    /// 但不带上它们的话，滚到行与行之间会缺一块。
    pub(super) fn visible(&self, scroll: f32, viewport: f32) -> Range<usize> {
        let first = self
            .cells
            .iter()
            .position(|cell| cell.y + self.row_height > scroll)
            .unwrap_or(self.cells.len());
        let last = self
            .cells
            .iter()
            .rposition(|cell| cell.y < scroll + viewport)
            .map_or(first, |index| index + 1);
        first..last.max(first)
    }

    /// 点 `(x, y)`（画布坐标，`scroll` 是当前位移）落在第几格。
    ///
    /// 落在格与格之间的缝上返回 `None`——那不是任何一个候选，点它不该上屏。
    pub fn hit(&self, x: f32, y: f32, scroll: f32) -> Option<usize> {
        let y = y + scroll;
        self.cells.iter().position(|cell| {
            x >= cell.x && x < cell.x + cell.width && y >= cell.y && y < cell.y + self.row_height
        })
    }
}

impl Renderer {
    /// 整份候选折成多行网格后的位置。
    ///
    /// 与候选条同源：格宽走 `bar_cell_widths`，行高走候选行那一条，
    /// 留白与格间缝也照主题。**组句一变就要重算一次**（量的是全部候选）。
    /// 渲染器不可用时给空的那份，会话据此画不出面板——与没有渲染器时的表现一致。
    pub fn candidate_grid(
        &mut self,
        rows: &[Row],
        content_width: f32,
        theme: &Theme,
        scale: f32,
    ) -> CandidateGrid {
        let m = Metrics { theme, scale };
        let widths = self.bar_cell_widths(rows, &m);
        CandidateGrid::new(
            &widths,
            content_width,
            m.px(super::super::bar::candidate_row_height(theme)),
            m.column_gap(),
            m.padding(),
            m.column_gap(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::CandidateGrid;

    /// 十格、每格 100 宽，放在 320 宽的板上（两边各留 10 点）。
    ///
    /// 可用宽 300、格间缝 10：一行正好摆下两格（100 + 10 + 100 = 210，再加一格就 320 > 310）。
    fn two_per_row() -> CandidateGrid {
        let widths = [100.0; 10];
        CandidateGrid::new(&widths, 320.0, 40.0, 10.0, 10.0, 10.0)
    }

    #[test]
    fn cells_wrap_onto_the_next_line() {
        let grid = two_per_row();
        assert_eq!(grid.len(), 10);
        assert_eq!(grid.cell(0).map(|c| c.x), Some(10.0));
        assert_eq!(grid.cell(1).map(|c| c.x), Some(120.0));
        // 第三格回到左边、往下走一行
        assert_eq!(grid.cell(2).map(|c| (c.x, c.y)), Some((10.0, 60.0)));
        assert_eq!(grid.cell(3).map(|c| c.x), Some(120.0));
    }

    #[test]
    fn the_total_height_covers_every_line() {
        // 10 格、一行 2 个 = 5 行，行高 40、行间缝 10、上下留白各 10：
        // 顶边 10 + 4 行 × (40 + 10) + 最后一行 40 + 下留白 10 = 260
        assert_eq!(two_per_row().total_height(), 260.0);
        assert_eq!(
            CandidateGrid::default().total_height(),
            0.0,
            "没有候选就不占地方"
        );
    }

    #[test]
    fn a_long_word_is_clipped_to_one_line() {
        // 一格 900 宽，板子只有 320：夹到可用宽 300，不夹的话这一行会被撑出屏幕
        let grid = CandidateGrid::new(&[900.0, 100.0], 320.0, 40.0, 10.0, 10.0, 10.0);
        assert_eq!(grid.cell(0).map(|c| c.width), Some(300.0));
        // 夹过之后它正好占满这一行（10 + 300 贴着右边那个 10），后面那格只能换行
        assert_eq!(grid.cell(1).map(|c| (c.x, c.y)), Some((10.0, 60.0)));
    }

    #[test]
    fn a_short_content_cannot_scroll() {
        let grid = CandidateGrid::new(&[100.0], 320.0, 40.0, 10.0, 10.0, 10.0);
        // 一格：10 + 40 + 10 = 60
        assert_eq!(grid.total_height(), 60.0);
        assert_eq!(grid.max_scroll(500.0), 0.0, "一屏装得下就不许滚");
        assert_eq!(grid.max_scroll(60.0), 0.0);
        assert_eq!(grid.max_scroll(20.0), 40.0);
    }

    #[test]
    fn only_the_visible_rows_are_drawn() {
        let grid = two_per_row();
        // 视口 100 高、停在第二行（y = 60）上：看得见第二、三行，第一行已经滚上去了
        let range = grid.visible(60.0, 100.0);
        assert_eq!(range, 2..6, "第二、三行各两格");
    }

    #[test]
    fn hit_lands_on_the_cell_under_the_finger() {
        let grid = two_per_row();
        // 第一行第一格
        assert_eq!(grid.hit(50.0, 20.0, 0.0), Some(0));
        // 第一行第二格
        assert_eq!(grid.hit(150.0, 20.0, 0.0), Some(1));
        // 格与格之间的缝（x = 105~120）落空
        assert_eq!(grid.hit(112.0, 20.0, 0.0), None);
        // 行与行之间的缝（y = 50~60）落空
        assert_eq!(grid.hit(50.0, 55.0, 0.0), None);
    }

    #[test]
    fn a_scrolled_hit_uses_the_content_position() {
        let grid = two_per_row();
        // 画布上同一点（右边那格、y = 20），滚与不滚差的就是位移那 60：
        // 不滚落在第一行右格（1），滚过一行落在第二行右格（3）
        assert_eq!(grid.hit(150.0, 20.0, 0.0), Some(1), "不滚时是第一行的右格");
        assert_eq!(
            grid.hit(150.0, 20.0, 60.0),
            Some(3),
            "滚过一行之后是第二行的右格"
        );
        // 贴到画布顶边那点（y = 5）：不滚时还在上留白里，够不着任何一格
        assert_eq!(grid.hit(150.0, 5.0, 0.0), None);
        assert_eq!(grid.hit(150.0, 5.0, 60.0), Some(3));
    }
}
