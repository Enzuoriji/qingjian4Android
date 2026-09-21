//! 青简那个标：四片竹简，用 App 图标那套绿。候选条没组句时，那条细的最左边画的就是它。
//!
//! 与 [`crate::gear`] 同一个路子——**画路径，不画字形**（`U+1F4DC` 这类字符会落进 emoji
//! 字体、或者干脆缺字，画出来不可控）。路径数据由 `assets/icon/render-logo-path.py`
//! 从 `assets/icon/menu.svg` 生成（见 [`path`]），这里只管怎么用。
//!
//! **形状用菜单栏那个（四片竹简）**：App 图标（`logo.png` / Windows 的 `qingjian.ico`）
//! 是 2 列 × 3 行**六片**细竹简、内容框 1:3.4 的竖条，缩到这条 30 点高的条子里每片只剩
//! 4×7 点，会糊成一团。颜色则是 App 那套绿（亮 `#94BE52`、深 `#336F33`）。
//!
//! 画的时候按**竹简自己的包围盒**缩（[`path::SLIPS_BOX`]），不是按 svg 的 viewBox——
//! 那 39×28 里竹简只占中间一小块，照 viewBox 缩的话四周全是留白、标看着就小。
//! 键帽不画（用户 2026-09-21 要去掉那个灰框）。

mod path;

use tiny_skia::{BlendMode, Pixmap, Transform};

use crate::canvas::Canvas;
use crate::color::Color;

/// 竹简的绿：亮的那几片。
const SLIP_LIGHT: Color = Color::rgb(0x94, 0xBE, 0x52);

/// 竹简的绿：深的那一片（App 图标里中间有一片是这个深绿）。
const SLIP_DARK: Color = Color::rgb(0x33, 0x6F, 0x33);

/// 四片竹简各是什么颜色，顺序与 [`path::SLIPS`] 一致（左上那片深的，其余亮的）。
const SLIP_COLORS: [Color; 4] = [SLIP_LIGHT, SLIP_DARK, SLIP_LIGHT, SLIP_LIGHT];

/// 按目标**高度**画这个标（竹简本身的高度，不含留白），左上角落在 `(x, y)`，返回占的宽度。
pub(crate) fn draw_logo(canvas: &mut Canvas, x: f32, y: f32, height: f32) -> f32 {
    let (left, top, right, bottom) = path::SLIPS_BOX;
    let scale = height / (bottom - top);
    let width = (right - left) * scale;
    let Some(bitmap) = Pixmap::new(width.ceil() as u32 + 1, height.ceil() as u32 + 1) else {
        return width;
    };
    let mut canvas_layer = Canvas::from_pixmap(bitmap);
    // 先把包围盒的左上角挪到原点，再按目标高度放大
    let transform = Transform::from_scale(scale, scale).post_translate(-left * scale, -top * scale);

    for (slip, slip_color) in path::SLIPS.into_iter().zip(SLIP_COLORS) {
        if let Some(slip) = slip().and_then(|path| path.transform(transform)) {
            canvas_layer.fill_path(&slip, slip_color, BlendMode::SourceOver);
        }
    }

    canvas.blend_pixmap(
        x.round() as i32,
        y.round() as i32,
        &canvas_layer.into_pixmap(),
    );
    width
}
