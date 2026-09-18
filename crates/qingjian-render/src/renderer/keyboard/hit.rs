//! 一个按键的命中矩形（键盘内容坐标，像素）。

use crate::keyboard::KeyId;

/// 某个键在键盘里的位置与大小。
#[derive(Debug, Clone, Copy)]
pub struct KeyHit {
    pub id: KeyId,

    pub x: f32,

    pub y: f32,

    pub width: f32,

    pub height: f32,
}

impl KeyHit {
    /// 点 `(x, y)`（键盘内容坐标）落在不落在这个键里。
    ///
    /// 右边界与下边界算开区间：相邻两键共用的那条线只归左边 / 上边那个键，
    /// 免得分界上的点有两个答案。
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }
}
