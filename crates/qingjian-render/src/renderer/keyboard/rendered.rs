//! 键盘的渲染结果：位图加每个键的命中矩形。

use super::hit::KeyHit;
use crate::keyboard::KeyId;
use crate::renderer::Rendered;

pub struct RenderedKeyboard {
    pub rendered: Rendered,

    /// 每个键的位置，顺序与布局里的按键一致。
    pub keys: Vec<KeyHit>,
}

impl RenderedKeyboard {
    /// 命中哪个键。
    ///
    /// `x` / `y` 是**位图像素坐标**（含内容区偏移），内部会先减掉 `content_x` / `content_y`。
    /// 落在键之间的缝隙上返回 `None`——那不是任何一个键。
    pub fn hit(&self, x: f32, y: f32) -> Option<KeyId> {
        let x = x - self.rendered.content_x as f32;
        let y = y - self.rendered.content_y as f32;
        self.keys
            .iter()
            .find(|key| key.contains(x, y))
            .map(|key| key.id)
    }
}
