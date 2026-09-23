//! 展开面板的渲染结果：位图加那张网格。

use super::grid::CandidateGrid;
use crate::renderer::Rendered;

pub struct RenderedPanel {
    pub rendered: Rendered,

    /// 这一帧是按哪张网格铺的。
    ///
    /// 格子位置**不另存一份**（没有「命中矩形」那种东西）：命中、滚动上限都从这张网格上问，
    /// 一份几何只有一个来源，两边不会各算各的。
    pub grid: CandidateGrid,
}

impl RenderedPanel {
    /// 命中哪一格，返回它是**整份候选列表**里的第几个。
    ///
    /// `x` / `y` 是**位图像素坐标**（含内容区偏移），`scroll` 是画这一帧时用的纵向位移——
    /// 网格里存的是内容坐标，得把它加回去才对得上手指看到的那一屏。
    /// 落在格与格之间的缝上返回 `None`——那不是任何一个候选，点它不该上屏。
    pub fn hit(&self, x: f32, y: f32, scroll: f32) -> Option<usize> {
        let x = x - self.rendered.content_x as f32;
        let y = y - self.rendered.content_y as f32;
        self.grid.hit(x, y, scroll)
    }

    /// 视口这么高时最远能滚到哪儿（像素）。
    pub fn max_scroll(&self, viewport: f32) -> f32 {
        self.grid.max_scroll(viewport)
    }
}
