//! 青简那个标：一枚键帽里嵌四片竹简。候选条没组句时，那条细的最左边画的就是它。
//!
//! 与 [`crate::gear`] 同一个路子——**画路径，不画字形**（`U+1F4DC` 这类字符会落进 emoji
//! 字体、或者干脆缺字，画出来不可控）。路径数据由 `assets/icon/render-logo-path.py`
//! 从 `assets/icon/menu.svg` 生成（即 macOS 菜单栏那个图标，见 [`path`]），这里只管怎么用。
//!
//! **竹简是镂空的**，所以先画在一张独立的小图上、在图上把竹简打透，再整张叠到画布
//! （与齿轮那个圆孔一个做法）——直接在画布上挖，会把候选条自己的背景也挖穿。

mod path;

use tiny_skia::{BlendMode, Pixmap, Transform};

use crate::canvas::Canvas;
use crate::color::Color;

/// svg 的 viewBox：39 宽 × 28 高。画多大都按这个比例缩。
const VIEW_WIDTH: f32 = 39.0;
const VIEW_HEIGHT: f32 = 28.0;

/// 按目标**高度**画这个标，左上角落在 `(x, y)`，用 `color` 上色，返回占的宽度。
pub(crate) fn draw_logo(canvas: &mut Canvas, x: f32, y: f32, height: f32, color: Color) -> f32 {
    let scale = height / VIEW_HEIGHT;
    let width = VIEW_WIDTH * scale;
    // 独立一张图：镂空是「在这张图上打透」，叠到画布上才是洞
    let Some(bitmap) = Pixmap::new(width.ceil() as u32 + 1, height.ceil() as u32 + 1) else {
        return width;
    };
    let mut layer = Canvas::from_pixmap(bitmap);
    let transform = Transform::from_scale(scale, scale);

    if let Some(keycap) = path::keycap().and_then(|path| path.transform(transform)) {
        layer.fill_path(&keycap, color, BlendMode::SourceOver);
    }
    for slip in path::SLIPS {
        if let Some(slip) = slip().and_then(|path| path.transform(transform)) {
            layer.fill_path(&slip, Color::rgb(0, 0, 0), BlendMode::Clear);
        }
    }

    canvas.blend_pixmap(x.round() as i32, y.round() as i32, &layer.into_pixmap());
    width
}
