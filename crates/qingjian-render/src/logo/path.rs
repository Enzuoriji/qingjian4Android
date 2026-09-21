//! 青简那个标的两部分路径：**一枚键帽 + 四片竹简**（竹简是挖空的孔）。
//!
//! **这个文件是生成的**，路径数据来自 `assets/icon/menu.svg`：
//!
//! ```sh
//! python assets/icon/render-logo-path.py
//! ```
//!
//! 改了 svg 就重跑那条命令，别手改这里。坐标是 svg 自己的 viewBox（39×28）。
//! 怎么用见 [`super::draw_logo`]。
//!
//! 坐标是量出来的数字，`3.14` 这种不是「圆周率写个大概」——所以关掉那条 lint。
#![allow(clippy::approx_constant)]

use tiny_skia::{Path, PathBuilder};

/// 键帽的外形（39×28 的圆角矩形）。
pub(super) fn keycap() -> Option<Path> {
    let mut builder = PathBuilder::new();
    builder.move_to(31.0, 0.0);
    builder.line_to(8.0, 0.0);
    builder.cubic_to(3.58, 0.0, 0.0, 3.58, 0.0, 8.0);
    builder.line_to(0.0, 20.0);
    builder.cubic_to(0.0, 24.42, 3.58, 28.0, 8.0, 28.0);
    builder.line_to(31.0, 28.0);
    builder.cubic_to(35.42, 28.0, 39.0, 24.42, 39.0, 20.0);
    builder.line_to(39.0, 8.0);
    builder.cubic_to(39.0, 3.58, 35.42, 0.0, 31.0, 0.0);
    builder.close();
    builder.finish()
}

/// 第 1 片竹简——**挖空**用的（见 `draw_logo`）。
pub(super) fn slip0() -> Option<Path> {
    let mut builder = PathBuilder::new();
    builder.move_to(20.61, 3.31);
    builder.cubic_to(20.59, 3.18, 20.8, 3.09, 20.98, 3.14);
    builder.cubic_to(21.72, 3.34, 22.43, 3.41, 23.05, 3.41);
    builder.cubic_to(23.68, 3.41, 24.39, 3.34, 25.13, 3.14);
    builder.cubic_to(25.31, 3.09, 25.52, 3.19, 25.5, 3.31);
    builder.cubic_to(25.37, 4.02, 25.07, 5.86, 25.09, 8.03);
    builder.cubic_to(25.07, 10.2, 25.37, 12.05, 25.5, 12.75);
    builder.cubic_to(25.52, 12.88, 25.31, 12.97, 25.13, 12.92);
    builder.cubic_to(24.39, 12.72, 23.68, 12.65, 23.05, 12.66);
    builder.line_to(23.05, 12.65);
    builder.cubic_to(22.43, 12.65, 21.72, 12.72, 20.98, 12.92);
    builder.cubic_to(20.8, 12.97, 20.59, 12.88, 20.61, 12.75);
    builder.cubic_to(20.74, 12.05, 21.03, 10.2, 21.02, 8.03);
    builder.cubic_to(21.03, 5.86, 20.74, 4.01, 20.61, 3.31);
    builder.close();
    builder.finish()
}

/// 第 2 片竹简——**挖空**用的（见 `draw_logo`）。
pub(super) fn slip1() -> Option<Path> {
    let mut builder = PathBuilder::new();
    builder.move_to(13.51, 3.35);
    builder.cubic_to(13.48, 3.22, 13.69, 3.13, 13.88, 3.18);
    builder.cubic_to(14.57, 3.37, 15.23, 3.44, 15.83, 3.44);
    builder.line_to(15.95, 3.44);
    builder.cubic_to(16.57, 3.45, 17.28, 3.38, 18.02, 3.18);
    builder.cubic_to(18.2, 3.13, 18.41, 3.22, 18.39, 3.35);
    builder.cubic_to(18.26, 4.05, 17.97, 5.9, 17.98, 8.07);
    builder.cubic_to(17.97, 10.24, 18.26, 12.08, 18.39, 12.78);
    builder.cubic_to(18.41, 12.91, 18.2, 13.01, 18.02, 12.96);
    builder.cubic_to(17.28, 12.76, 16.57, 12.69, 15.95, 12.69);
    builder.cubic_to(15.32, 12.69, 14.61, 12.76, 13.88, 12.95);
    builder.cubic_to(13.69, 13.01, 13.48, 12.91, 13.51, 12.78);
    builder.cubic_to(13.63, 12.08, 13.93, 10.24, 13.91, 8.07);
    builder.cubic_to(13.93, 5.9, 13.63, 4.05, 13.51, 3.35);
    builder.close();
    builder.finish()
}

/// 第 3 片竹简——**挖空**用的（见 `draw_logo`）。
pub(super) fn slip2() -> Option<Path> {
    let mut builder = PathBuilder::new();
    builder.move_to(18.18, 24.86);
    builder.cubic_to(17.45, 24.66, 16.74, 24.59, 16.11, 24.59);
    builder.cubic_to(16.06, 24.59, 16.0, 24.59, 15.95, 24.6);
    builder.cubic_to(15.89, 24.59, 15.83, 24.59, 15.78, 24.59);
    builder.cubic_to(15.15, 24.59, 14.44, 24.66, 13.71, 24.86);
    builder.cubic_to(13.52, 24.91, 13.32, 24.82, 13.34, 24.69);
    builder.cubic_to(13.47, 23.99, 13.76, 22.14, 13.74, 19.97);
    builder.cubic_to(13.76, 18.04, 13.53, 16.36, 13.39, 15.52);
    builder.cubic_to(13.36, 15.35, 13.5, 15.19, 13.75, 15.12);
    builder.cubic_to(14.2, 14.99, 14.99, 14.82, 15.95, 14.83);
    builder.cubic_to(16.9, 14.82, 17.69, 14.99, 18.15, 15.12);
    builder.cubic_to(18.39, 15.19, 18.53, 15.35, 18.51, 15.52);
    builder.cubic_to(18.37, 16.36, 18.13, 18.04, 18.15, 19.97);
    builder.cubic_to(18.13, 22.14, 18.43, 23.99, 18.55, 24.69);
    builder.cubic_to(18.58, 24.82, 18.37, 24.91, 18.18, 24.86);
    builder.close();
    builder.finish()
}

/// 第 4 片竹简——**挖空**用的（见 `draw_logo`）。
pub(super) fn slip3() -> Option<Path> {
    let mut builder = PathBuilder::new();
    builder.move_to(25.29, 24.82);
    builder.cubic_to(24.56, 24.62, 23.85, 24.55, 23.22, 24.56);
    builder.line_to(22.77, 24.56);
    builder.cubic_to(22.17, 24.56, 21.51, 24.63, 20.82, 24.82);
    builder.cubic_to(20.63, 24.87, 20.42, 24.78, 20.45, 24.65);
    builder.cubic_to(20.57, 23.95, 20.87, 22.1, 20.85, 19.93);
    builder.cubic_to(20.87, 18.0, 20.63, 16.32, 20.49, 15.49);
    builder.cubic_to(20.47, 15.32, 20.61, 15.15, 20.85, 15.08);
    builder.cubic_to(21.31, 14.96, 22.1, 14.78, 23.05, 14.8);
    builder.cubic_to(24.01, 14.78, 24.8, 14.96, 25.25, 15.08);
    builder.cubic_to(25.5, 15.15, 25.64, 15.32, 25.61, 15.49);
    builder.cubic_to(25.47, 16.32, 25.24, 18.0, 25.26, 19.93);
    builder.cubic_to(25.24, 22.1, 25.53, 23.95, 25.66, 24.65);
    builder.cubic_to(25.68, 24.78, 25.48, 24.87, 25.29, 24.82);
    builder.close();
    builder.finish()
}

/// 四片竹简，`slip0` 起。
pub(super) const SLIPS: [fn() -> Option<Path>; 4] = [slip0, slip1, slip2, slip3];
