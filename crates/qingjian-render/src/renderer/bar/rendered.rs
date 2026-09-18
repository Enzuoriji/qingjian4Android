//! 候选条的渲染结果：位图加每块可点区域。

use super::hit::BarHit;
use super::id::BarHitId;
use crate::renderer::Rendered;

pub struct RenderedBar {
    pub rendered: Rendered,

    /// 本条候选条上可点的区域。候选为空时只有背景，这里是空的。
    pub hits: Vec<BarHit>,
}

impl RenderedBar {
    /// 命中哪一块。
    ///
    /// `x` / `y` 是**位图像素坐标**（含内容区偏移），内部先减掉 `content_x` / `content_y`。
    /// 落在块与块之间返回 `None`——那不是任何一个目标。
    pub fn hit(&self, x: f32, y: f32) -> Option<BarHitId> {
        let x = x - self.rendered.content_x as f32;
        let y = y - self.rendered.content_y as f32;
        self.hits
            .iter()
            .find(|hit| hit.contains(x, y))
            .map(|hit| hit.id)
    }
}
