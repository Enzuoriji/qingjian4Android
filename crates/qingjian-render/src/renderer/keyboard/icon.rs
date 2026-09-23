//! 键盘上那几个图标：⇧ 大小写、⌫ 退格，工具页那几格上的剪贴板 / 表情 / 颜文字 / 设置，
//! 以及表情面板分类标签上那一排（2026-09-23 加的）。
//!
//! **路径是生成的**，来自 `assets/icon/material/` 里那几张 Google **Material Symbols** 的 svg
//! （Apache-2.0）：
//!
//! ```sh
//! python assets/icon/render-key-icon-path.py
//! ```
//!
//! 写出 [`path`]，生成物随仓库提交。**别手写坐标**——2026-09-21 之前这几个图标是手写的
//! `move_to` / `line_to`，用户看了说「不要这样做去网上找可以用的」，于是换成现成的。
//!
//! 这里只做一件事：把那段路径按**它自己的包围盒**缩成目标边长、挪到要画的位置、填色。
//! 与 [`crate::logo`] 那套同一个路数。Material 的图标靠**子路径方向**挖空（外轮廓顺时针、
//! 内轮廓逆时针），所以直接按默认的非零规则填就行，不必自己打孔。
//!
//! 画路径不画字形这条理由与 [`crate::gear`] 一样：`U+21E7`（⇧）这类符号在某些设备上会落进
//! 彩色 emoji 字体或被回退链吃掉，画出来不可控。

mod path;

use tiny_skia::{BlendMode, Transform};

use crate::canvas::Canvas;
use crate::color::Color;
use crate::keyboard::GroupIcon;

/// 图标边长相对**键高**的比例。
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

/// 键帽上那个图标该画多大（像素）。`key_height` 是键高（像素），`scale` 是屏幕密度。
pub(super) fn size_on_key(key_height: f32, scale: f32) -> f32 {
    (key_height * SIZE_RATIO).clamp(MIN_SIZE * scale, MAX_SIZE * scale)
}

/// 画上档（大小写）图标，居中在 `(cx, cy)`，边长 `size` 像素。
pub(crate) fn draw_shift(canvas: &mut Canvas, cx: f32, cy: f32, size: f32, color: Color) {
    draw(canvas, path::shift(), path::BOXES[0], cx, cy, size, color);
}

/// 画退格 ⌫，居中在 `(cx, cy)`，边长 `size` 像素。
pub(crate) fn draw_backspace(canvas: &mut Canvas, cx: f32, cy: f32, size: f32, color: Color) {
    draw(
        canvas,
        path::backspace(),
        path::BOXES[1],
        cx,
        cy,
        size,
        color,
    );
}

/// 画剪贴板（工具页那一格），居中在 `(cx, cy)`，边长 `size` 像素。
pub(crate) fn draw_clipboard(canvas: &mut Canvas, cx: f32, cy: f32, size: f32, color: Color) {
    draw(
        canvas,
        path::clipboard(),
        path::BOXES[2],
        cx,
        cy,
        size,
        color,
    );
}

/// 画表情（工具页那一格），居中在 `(cx, cy)`，边长 `size` 像素。
pub(crate) fn draw_mood(canvas: &mut Canvas, cx: f32, cy: f32, size: f32, color: Color) {
    draw(canvas, path::mood(), path::BOXES[3], cx, cy, size, color);
}

/// 画颜文字（工具页那一格），居中在 `(cx, cy)`，边长 `size` 像素。
pub(crate) fn draw_kaomoji(canvas: &mut Canvas, cx: f32, cy: f32, size: f32, color: Color) {
    draw(canvas, path::kaomoji(), path::BOXES[4], cx, cy, size, color);
}

/// 画设置（工具页那一格），居中在 `(cx, cy)`，边长 `size` 像素。
pub(crate) fn draw_settings(canvas: &mut Canvas, cx: f32, cy: f32, size: f32, color: Color) {
    draw(
        canvas,
        path::settings(),
        path::BOXES[5],
        cx,
        cy,
        size,
        color,
    );
}

/// 画表情面板分类标签上那个图标，居中在 `(cx, cy)`，边长 `size` 像素。
///
/// 「哪个分类用哪个图标」由会话层定（见 [`GroupIcon`]），这儿只认枚举值。
pub(crate) fn draw_group(
    canvas: &mut Canvas,
    icon: GroupIcon,
    cx: f32,
    cy: f32,
    size: f32,
    color: Color,
) {
    let (path, bbox) = match icon {
        GroupIcon::Recent => (path::history(), path::BOXES[6]),
        // 笑脸那格复用工具页的表情图标：同一个意思，没必要再下一张
        GroupIcon::Smile => (path::mood(), path::BOXES[3]),
        GroupIcon::People => (path::people(), path::BOXES[7]),
        GroupIcon::Animals => (path::pets(), path::BOXES[8]),
        GroupIcon::Food => (path::cake(), path::BOXES[9]),
        GroupIcon::Travel => (path::car(), path::BOXES[10]),
        GroupIcon::Activities => (path::ball(), path::BOXES[11]),
        GroupIcon::Objects => (path::objects(), path::BOXES[12]),
        GroupIcon::Symbols => (path::symbols(), path::BOXES[13]),
        GroupIcon::Flags => (path::flag(), path::BOXES[14]),
    };
    draw(canvas, path, bbox, cx, cy, size, color);
}

/// 把一段路径缩到**最长边 = `size`** 并居中在 `(cx, cy)`，然后填色。
///
/// 按最长边而不是按高：这几个图标有宽扁的（⌫ 的包围盒是 800×640），按高缩会顶出格子。
/// 按**各自包围盒**而不是 Material 那个 960 的网格缩：网格四周是 Google 留的呼吸位，
/// 照网格缩的话画出来比要的尺寸小一圈。
fn draw(
    canvas: &mut Canvas,
    path: Option<tiny_skia::Path>,
    bbox: (f32, f32, f32, f32),
    cx: f32,
    cy: f32,
    size: f32,
    color: Color,
) {
    let Some(path) = path else {
        return;
    };
    let (left, top, right, bottom) = bbox;
    let (width, height) = (right - left, bottom - top);
    if width <= 0.0 || height <= 0.0 || size <= 0.0 {
        return;
    }
    let scale = size / width.max(height);
    let transform = Transform::from_scale(scale, scale).post_translate(
        cx - (left + right) / 2.0 * scale,
        cy - (top + bottom) / 2.0 * scale,
    );
    if let Some(path) = path.transform(transform) {
        canvas.fill_path(&path, color, BlendMode::SourceOver);
    }
}
