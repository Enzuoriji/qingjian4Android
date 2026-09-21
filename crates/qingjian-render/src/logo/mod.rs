//! 青简那个标：一枚键帽里嵌四片竹简。候选条没组句时，那条细的最左边画的就是它。
//!
//! 与 [`crate::gear`] 同一个路子——**画路径，不画字形**（`U+1F4DC` 这类字符会落进 emoji
//! 字体、或者干脆缺字，画出来不可控）。路径数据由 `assets/icon/render-logo-path.py`
//! 从 `assets/icon/menu.svg` 生成（即 macOS 菜单栏那个图标，见 [`path`]），这里只管怎么用。
//!
//! **竹简用 App 图标那套绿**（`assets/icon/logo.png` / Windows 的 `qingjian.ico` 是同一个）：
//! 键帽是「形」、跟候选条同色（明暗主题都看得见），竹简是「色」、就是品牌那几种绿。
//! 形状仍用菜单栏那个 39×28 的——App 图标是 2 列 × 3 行六片细竹简（内容框 1:3.4 的竖条），
//! 缩到这条 30 点高的细条里每片只有 4×7 点，会糊成一团。
//!
//! 先画在一张独立的小图上再整张叠到画布：竹简是**填在键帽上**的，不在画布上直接画
//! 是为了跟 [`crate::gear`] 一样把「合成」和「贴上去」分开（也省得碰候选条自己的背景）。

mod path;

use tiny_skia::{BlendMode, Pixmap, Transform};

use crate::canvas::Canvas;
use crate::color::Color;

/// svg 的 viewBox：39 宽 × 28 高。画多大都按这个比例缩。
const VIEW_WIDTH: f32 = 39.0;
const VIEW_HEIGHT: f32 = 28.0;

/// 竹简的绿：亮的那几片。
const SLIP_LIGHT: Color = Color::rgb(0x94, 0xBE, 0x52);

/// 竹简的绿：深的那一片（App 图标里四片/六片中间有一片是这个深绿）。
const SLIP_DARK: Color = Color::rgb(0x33, 0x6F, 0x33);

/// 四片竹简各是什么颜色，顺序与 [`path::SLIPS`] 一致（左上那片深的，其余亮的）。
const SLIP_COLORS: [Color; 4] = [SLIP_LIGHT, SLIP_DARK, SLIP_LIGHT, SLIP_LIGHT];

/// 按目标**高度**画这个标，左上角落在 `(x, y)`，键帽用 `color`，返回占的宽度。
pub(crate) fn draw_logo(canvas: &mut Canvas, x: f32, y: f32, height: f32, color: Color) -> f32 {
    let scale = height / VIEW_HEIGHT;
    let width = VIEW_WIDTH * scale;
    let Some(bitmap) = Pixmap::new(width.ceil() as u32 + 1, height.ceil() as u32 + 1) else {
        return width;
    };
    let mut layer = Canvas::from_pixmap(bitmap);
    let transform = Transform::from_scale(scale, scale);

    // 键帽先铺满整块（它是「形」）
    if let Some(keycap) = path::keycap().and_then(|path| path.transform(transform)) {
        layer.fill_path(&keycap, color, BlendMode::SourceOver);
    }
    // 竹简填在键帽上面（「色」）——原来的镂空就此变成品牌绿
    for (slip, slip_color) in path::SLIPS.into_iter().zip(SLIP_COLORS) {
        if let Some(slip) = slip().and_then(|path| path.transform(transform)) {
            layer.fill_path(&slip, slip_color, BlendMode::SourceOver);
        }
    }

    canvas.blend_pixmap(x.round() as i32, y.round() as i32, &layer.into_pixmap());
    width
}
