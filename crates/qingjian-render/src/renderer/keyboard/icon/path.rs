//! 键盘上那几个图标的路径：⇧ 大小写、⌫ 退格、剪贴板（各一个函数 + 一个包围盒）。
//!
//! **这个文件是生成的**，路径数据来自 `assets/icon/material/` 里那几张 svg
//! （Google 的 Material Symbols，Apache-2.0）：
//!
//! ```sh
//! python assets/icon/render-key-icon-path.py
//! ```
//!
//! 换了图标就重跑那条命令，别手改这里。坐标是 Material 那套 960×960、y 轴朝上的网格
//! （所以 y 是负的），缩放与居中在 [`super`] 里做。
#![allow(clippy::approx_constant)]

use tiny_skia::{Path, PathBuilder};

/// 上档（大小写）——Material 的 `keyboard_capslock`。
pub(super) fn shift() -> Option<Path> {
    let mut builder = PathBuilder::new();
    builder.move_to(240.0, -240.0);
    builder.line_to(240.0, -320.0);
    builder.line_to(720.0, -320.0);
    builder.line_to(720.0, -240.0);
    builder.line_to(240.0, -240.0);
    builder.close();
    builder.move_to(480.0, -736.0);
    builder.line_to(720.0, -496.0);
    builder.line_to(664.0, -440.0);
    builder.line_to(480.0, -624.0);
    builder.line_to(296.0, -440.0);
    builder.line_to(240.0, -496.0);
    builder.line_to(480.0, -736.0);
    builder.close();
    builder.finish()
}

/// 退格——Material 的 `backspace`。
pub(super) fn backspace() -> Option<Path> {
    let mut builder = PathBuilder::new();
    builder.move_to(456.0, -320.0);
    builder.line_to(560.0, -424.0);
    builder.line_to(664.0, -320.0);
    builder.line_to(720.0, -376.0);
    builder.line_to(616.0, -480.0);
    builder.line_to(720.0, -584.0);
    builder.line_to(664.0, -640.0);
    builder.line_to(560.0, -536.0);
    builder.line_to(456.0, -640.0);
    builder.line_to(400.0, -584.0);
    builder.line_to(504.0, -480.0);
    builder.line_to(400.0, -376.0);
    builder.line_to(456.0, -320.0);
    builder.close();
    builder.move_to(360.0, -160.0);
    builder.quad_to(341.0, -160.0, 324.0, -168.5);
    builder.quad_to(307.0, -177.0, 296.0, -192.0);
    builder.line_to(80.0, -480.0);
    builder.line_to(296.0, -768.0);
    builder.quad_to(307.0, -783.0, 324.0, -791.5);
    builder.quad_to(341.0, -800.0, 360.0, -800.0);
    builder.line_to(800.0, -800.0);
    builder.quad_to(833.0, -800.0, 856.5, -776.5);
    builder.quad_to(880.0, -753.0, 880.0, -720.0);
    builder.line_to(880.0, -240.0);
    builder.quad_to(880.0, -207.0, 856.5, -183.5);
    builder.quad_to(833.0, -160.0, 800.0, -160.0);
    builder.line_to(360.0, -160.0);
    builder.close();
    builder.move_to(180.0, -480.0);
    builder.line_to(360.0, -240.0);
    builder.line_to(800.0, -240.0);
    builder.line_to(800.0, -720.0);
    builder.line_to(360.0, -720.0);
    builder.line_to(180.0, -480.0);
    builder.close();
    builder.move_to(580.0, -480.0);
    builder.close();
    builder.finish()
}

/// 剪贴板（工具页那一格）——Material 的 `content_paste`。
pub(super) fn clipboard() -> Option<Path> {
    let mut builder = PathBuilder::new();
    builder.move_to(200.0, -120.0);
    builder.quad_to(167.0, -120.0, 143.5, -143.5);
    builder.quad_to(120.0, -167.0, 120.0, -200.0);
    builder.line_to(120.0, -760.0);
    builder.quad_to(120.0, -793.0, 143.5, -816.5);
    builder.quad_to(167.0, -840.0, 200.0, -840.0);
    builder.line_to(367.0, -840.0);
    builder.quad_to(378.0, -875.0, 410.0, -897.5);
    builder.quad_to(442.0, -920.0, 480.0, -920.0);
    builder.quad_to(520.0, -920.0, 551.5, -897.5);
    builder.quad_to(583.0, -875.0, 594.0, -840.0);
    builder.line_to(760.0, -840.0);
    builder.quad_to(793.0, -840.0, 816.5, -816.5);
    builder.quad_to(840.0, -793.0, 840.0, -760.0);
    builder.line_to(840.0, -200.0);
    builder.quad_to(840.0, -167.0, 816.5, -143.5);
    builder.quad_to(793.0, -120.0, 760.0, -120.0);
    builder.line_to(200.0, -120.0);
    builder.close();
    builder.move_to(200.0, -200.0);
    builder.line_to(760.0, -200.0);
    builder.line_to(760.0, -760.0);
    builder.line_to(680.0, -760.0);
    builder.line_to(680.0, -640.0);
    builder.line_to(280.0, -640.0);
    builder.line_to(280.0, -760.0);
    builder.line_to(200.0, -760.0);
    builder.line_to(200.0, -200.0);
    builder.close();
    builder.move_to(508.5, -771.5);
    builder.quad_to(520.0, -783.0, 520.0, -800.0);
    builder.quad_to(520.0, -817.0, 508.5, -828.5);
    builder.quad_to(497.0, -840.0, 480.0, -840.0);
    builder.quad_to(463.0, -840.0, 451.5, -828.5);
    builder.quad_to(440.0, -817.0, 440.0, -800.0);
    builder.quad_to(440.0, -783.0, 451.5, -771.5);
    builder.quad_to(463.0, -760.0, 480.0, -760.0);
    builder.quad_to(497.0, -760.0, 508.5, -771.5);
    builder.close();
    builder.finish()
}

/// 每个图标自己的包围盒（Material 那套坐标）：`(左, 上, 右, 下)`。
///
/// 画的时候按**这个框**等比缩到目标边长——960 的网格里四周是 Google 留的呼吸位，
/// 照网格缩的话画出来比要的尺寸小一圈。
pub(super) const BOXES: [(f32, f32, f32, f32); 3] = [
    (240.0, -736.0, 720.0, -240.0), // shift
    (80.0, -800.0, 880.0, -160.0),  // backspace
    (120.0, -920.0, 840.0, -120.0), // clipboard
];
