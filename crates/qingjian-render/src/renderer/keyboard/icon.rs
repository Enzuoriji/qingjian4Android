//! 键帽上的图标：上档箭头与退格。
//!
//! 画路径不画字形——`U+21E7`（⇧）与 `U+232B`（⌫）这类符号在某些设备上会落进彩色 emoji
//! 字体或被回退链吃掉，画出来不可控。理由与 `gear.rs` 相同。
//!
//! 画法也照 `gear.rs`：先画进一张独立的小图，需要挖空的部分用 `BlendMode::Clear` 打透
//! （打透处透明，贴到键帽上就露出键帽自己的颜色），最后整张贴上去。直接在键盘画布上 Clear
//! 会把键盘挖出一个透明的洞，露出后面的应用。

use tiny_skia::{BlendMode, PathBuilder};

use crate::canvas::Canvas;
use crate::color::Color;

/// 图标边长相对键高的比例。
const SIZE_RATIO: f32 = 0.42;

/// 图标的最大边长（点）——键再大也不让图标跟着无限变大。
const MAX_SIZE: f32 = 20.0;

/// 画上档箭头 ⇧，居中在 `(cx, cy)`。
pub(crate) fn draw_shift(canvas: &mut Canvas, cx: f32, cy: f32, key_height: f32, color: Color) {
    let Some((mut layer, size)) = layer(key_height) else {
        return;
    };
    let s = size;
    let mut path = PathBuilder::new();
    path.move_to(s * 0.50, s * 0.10);
    path.line_to(s * 0.96, s * 0.52);
    path.line_to(s * 0.70, s * 0.52);
    path.line_to(s * 0.70, s * 0.90);
    path.line_to(s * 0.30, s * 0.90);
    path.line_to(s * 0.30, s * 0.52);
    path.line_to(s * 0.04, s * 0.52);
    path.close();
    if let Some(path) = path.finish() {
        layer.fill_path(&path, color, BlendMode::SourceOver);
    }
    canvas.blend_pixmap(
        (cx - s / 2.0) as i32,
        (cy - s / 2.0) as i32,
        &layer.into_pixmap(),
    );
}

/// 画退格 ⌫，居中在 `(cx, cy)`。
pub(crate) fn draw_backspace(canvas: &mut Canvas, cx: f32, cy: f32, key_height: f32, color: Color) {
    let Some((mut layer, size)) = layer(key_height) else {
        return;
    };
    let s = size;

    // 左端带尖角的五边形
    let mut path = PathBuilder::new();
    path.move_to(s * 0.04, s * 0.50);
    path.line_to(s * 0.34, s * 0.14);
    path.line_to(s * 0.96, s * 0.14);
    path.line_to(s * 0.96, s * 0.86);
    path.line_to(s * 0.34, s * 0.86);
    path.close();
    if let Some(path) = path.finish() {
        layer.fill_path(&path, color, BlendMode::SourceOver);
    }

    // 打透一个叉
    let (x, y) = (s * 0.65, s * 0.50);
    let (arm, thick) = (s * 0.13, s * 0.055);
    for flip in [1.0, -1.0] {
        let mut path = PathBuilder::new();
        path.move_to(x - arm * flip, y - arm);
        path.line_to(x - arm * flip + thick, y - arm);
        path.line_to(x + arm * flip + thick, y + arm);
        path.line_to(x + arm * flip, y + arm);
        path.close();
        if let Some(path) = path.finish() {
            layer.fill_path(&path, Color::rgb(0, 0, 0), BlendMode::Clear);
        }
    }
    canvas.blend_pixmap(
        (cx - s / 2.0) as i32,
        (cy - s / 2.0) as i32,
        &layer.into_pixmap(),
    );
}

/// 建一张空的小画布与它的边长（像素），给图标用。
fn layer(key_height: f32) -> Option<(Canvas, f32)> {
    let size = (key_height * SIZE_RATIO).clamp(8.0, MAX_SIZE).ceil();
    let canvas = Canvas::new(size as u32, size as u32).ok()?;
    Some((canvas, size))
}
