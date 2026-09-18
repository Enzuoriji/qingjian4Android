//! 候选条上一块可点区域的位置与大小（由[渲染](super::Renderer::render_bar)一并返回）。

use super::id::BarHitId;

/// 候选条里某个可点目标的位置与大小，坐标是**内容区**的像素。
#[derive(Debug, Clone, Copy)]
pub struct BarHit {
    pub id: BarHitId,

    pub x: f32,

    pub y: f32,

    pub width: f32,

    pub height: f32,
}

impl BarHit {
    /// 点 `(x, y)`（候选条内容坐标）落不落在这块里。
    ///
    /// 右 / 下边界是开区间，与键盘一致：相邻两块共用的那条线只归左边 / 上边那个，
    /// 免得分界上的点有两个答案。
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }
}
