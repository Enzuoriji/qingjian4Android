//! 键帽上的图标：上档箭头与退格。
//!
//! 画路径不画字形——`U+21E7`（⇧）与 `U+232B`（⌫）这类符号在某些设备上会落进彩色 emoji
//! 字体或被回退链吃掉，画出来不可控。理由与 `gear.rs` 相同。
//!
//! 画法也照 `gear.rs`：先画进一张独立的小图，需要挖空的部分用 `BlendMode::Clear` 打透
//! （打透处透明，贴到键帽上就露出键帽自己的颜色），最后整张贴上去。直接在键盘画布上 Clear
//! 会把键盘挖出一个透明的洞，露出后面的应用。

use tiny_skia::{BlendMode, PathBuilder};

use crate::canvas::{Canvas, round_rect};
use crate::color::Color;

/// 图标边长相对键高的比例。
///
/// **0.46 是照实机截图反推的**：参考图上 ⌫ 那个图标高占键高 0.33，而画法里图标
/// 只占这张方块的 0.72，0.33 ÷ 0.72 ≈ 0.46。
/// 试过 0.55（图标占键高 0.43），比字母还抢眼，退回来了。
const SIZE_RATIO: f32 = 0.46;

/// 图标边长的上限（**点**）。
///
/// 上下限都按点算，画的时候再乘密度——**按像素算是个坑**：键高是像素值、随密度涨，
/// 上限却钉死在像素上，于是**屏幕密度越高、图标相对越小**。真机上「图标偏小」就是这么来的。
const MAX_SIZE: f32 = 26.0;

/// 图标边长的下限（**点**）。键再小也别小到看不清。
const MIN_SIZE: f32 = 6.0;

/// 画上档箭头 ⇧，居中在 `(cx, cy)`。
pub(crate) fn draw_shift(
    canvas: &mut Canvas,
    cx: f32,
    cy: f32,
    key_height: f32,
    scale: f32,
    color: Color,
) {
    let Some((mut layer, size)) = layer(key_height, scale) else {
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
pub(crate) fn draw_backspace(
    canvas: &mut Canvas,
    cx: f32,
    cy: f32,
    key_height: f32,
    scale: f32,
    color: Color,
) {
    let Some((mut layer, size)) = layer(key_height, scale) else {
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

/// 画剪贴板（工具页那个格子上的图标），居中在 `(cx, cy)`，边长 `size` 像素。
///
/// 一块**板**加顶上一个**夹子**：板身画满再打透里面（剩一圈边，与 [`draw_backspace`]
/// 打那个叉同一个路数），夹子盖在顶边上，中间那两条短线是「一页字」。
///
/// 与另外两个图标不同，这个收的是**边长**而不是键高：工具页那种格子是「图标 + 名字」
/// 两行，图标多大由那儿算好，不是按「键高 × 比例」来的。
pub(crate) fn draw_clipboard(canvas: &mut Canvas, cx: f32, cy: f32, size: f32, color: Color) {
    let size = size.ceil().max(1.0);
    let Some(mut layer) = Canvas::new(size as u32, size as u32).ok() else {
        return;
    };
    let s = size;
    let stroke = (s * 0.11).max(1.0);
    let radius = s * 0.10;
    let (bx, by, bw, bh) = (s * 0.16, s * 0.12, s * 0.68, s * 0.84);

    // 板身：先整个画满
    if let Some(path) = round_rect(bx, by, bw, bh, radius) {
        layer.fill_path(&path, color, BlendMode::SourceOver);
    }
    // 再把里面打透，剩下一圈边
    if let Some(path) = round_rect(
        bx + stroke,
        by + stroke,
        bw - stroke * 2.0,
        bh - stroke * 2.0,
        (radius - stroke).max(0.0),
    ) {
        layer.fill_path(&path, Color::rgb(0, 0, 0), BlendMode::Clear);
    }
    // 顶上的夹子：压住上边那一段，看着才是「夹着的板」而不是一个空框
    if let Some(path) = round_rect(s * 0.35, s * 0.02, s * 0.30, s * 0.22, s * 0.05) {
        layer.fill_path(&path, color, BlendMode::SourceOver);
    }
    // 板子上那两行字
    for row in 0..2 {
        let y = s * (0.46 + row as f32 * 0.18);
        if let Some(path) = round_rect(s * 0.29, y, s * 0.42, stroke * 0.85, stroke * 0.4) {
            layer.fill_path(&path, color, BlendMode::SourceOver);
        }
    }

    canvas.blend_pixmap(
        (cx - s / 2.0) as i32,
        (cy - s / 2.0) as i32,
        &layer.into_pixmap(),
    );
}

/// 建一张空的小画布与它的边长（像素），给图标用。
///
/// `key_height` 是像素，`scale` 是屏幕密度——上下限按点写，乘上 `scale` 才跟键高同一个量纲。
fn layer(key_height: f32, scale: f32) -> Option<(Canvas, f32)> {
    let size = (key_height * SIZE_RATIO)
        .clamp(MIN_SIZE * scale, MAX_SIZE * scale)
        .ceil();
    let canvas = Canvas::new(size as u32, size as u32).ok()?;
    Some((canvas, size))
}
