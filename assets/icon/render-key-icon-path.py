#!/usr/bin/env python3
"""把 `assets/icon/material/` 那几个 SVG 转成 Rust 的 `PathBuilder` 调用。

键盘上那几个图标（⇧ 大小写、⌫ 退格、剪贴板）用的就是它——**画路径不画字形**，
理由与 `gear.rs` 一样：`U+21E7` 这类字符会落进 emoji 字体或者缺字，画出来不可控。

    python assets/icon/render-key-icon-path.py

写出 `crates/qingjian-render/src/renderer/keyboard/icon/path.rs`，生成物**随仓库提交**。
换了图标（改 `material/` 里那几张 svg、或者加新的）就重跑这条命令，别手改生成物。

图标是 Google 的 **Material Symbols**（Apache-2.0，见 `assets/icon/README.md`），
所以坐标是它那套 960×960、**y 轴朝上为负**的网格——原样搬过来，缩放交给渲染器
（按每个图标自己的包围盒缩，见 `icon/mod.rs`）。
"""
import pathlib
import sys

HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

from svgpath import bbox_of, d_of, num, parse  # noqa: E402

OUT = HERE.parents[1] / "crates/qingjian-render/src/renderer/keyboard/icon/path.rs"

# 要生成哪几个图标：`(Rust 里的名字, svg 文件名, 说明)`
ICONS = [
    ("shift", "keyboard_capslock.svg", "上档（大小写）——Material 的 `keyboard_capslock`"),
    ("backspace", "backspace.svg", "退格——Material 的 `backspace`"),
    ("clipboard", "content_paste.svg", "剪贴板（工具页那一格）——Material 的 `content_paste`"),
    ("mood", "mood.svg", "表情（工具页那一格）——Material 的 `mood`"),
    (
        "kaomoji",
        "sentiment_satisfied.svg",
        "颜文字（工具页那一格）——Material 的 `sentiment_satisfied`",
    ),
    ("settings", "settings.svg", "设置（工具页那一格）——Material 的 `settings`"),
    # 表情面板分类标签上那一排（2026-09-23，照 fcitx5-android 的选型）。
    # 颜文字那 22 个中文分类**不用图标**：一行挤 22 个，图标小到认不出，那边画文字。
    ("history", "history.svg", "「最近」那一类——Material 的 `history`"),
    ("people", "emoji_people.svg", "「People & Body」——Material 的 `emoji_people`"),
    ("pets", "pets.svg", "「Animals & Nature」——Material 的 `pets`"),
    ("cake", "cake.svg", "「Food & Drink」——Material 的 `cake`"),
    ("car", "directions_car.svg", "「Travel & Places」——Material 的 `directions_car`"),
    (
        "ball",
        "sports_basketball.svg",
        "「Activities」——Material 的 `sports_basketball`",
    ),
    ("objects", "emoji_objects.svg", "「Objects」——Material 的 `emoji_objects`"),
    ("symbols", "emoji_symbols.svg", "「Symbols」——Material 的 `emoji_symbols`"),
    ("flag", "flag.svg", "「Flags」——Material 的 `flag`"),
]


def emit_call(name, args):
    return f"    builder.{name}({', '.join(num(a) for a in args)});"


def emit_function(name, doc, segments):
    lines = [f"/// {doc}", f"pub(super) fn {name}() -> Option<Path> {{"]
    lines.append("    let mut builder = PathBuilder::new();")
    lines += [emit_call(cmd, args) for cmd, args in segments]
    lines.append("    builder.finish()")
    lines.append("}")
    return "\n".join(lines)


def main():
    header = '''//! 键盘上那些图标的路径：⇧ 大小写、⌫ 退格、工具页那几格，
//! 以及表情面板分类标签上那一排（各一个函数 + 一个包围盒）。
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
'''
    body = []
    boxes = []
    for name, file, doc in ICONS:
        svg = (HERE / "material" / file).read_text(encoding="utf-8")
        paths = parse(d_of(svg))
        # 一张图标可能是好几段子路径（⇧ 就是「箭头 + 底下一横」两段），
        # 一条命令一条命令地接着写进同一个 builder 里——`PathBuilder` 本来就收多段
        segments = [segment for sub in paths for segment in sub]
        body.append(emit_function(name, f"{doc}。", segments))
        boxes.append((name, bbox_of(paths)))

    body.append(
        """/// 每个图标自己的包围盒（Material 那套坐标）：`(左, 上, 右, 下)`。
///
/// 画的时候按**这个框**等比缩到目标边长——960 的网格里四周是 Google 留的呼吸位，
/// 照网格缩的话画出来比要的尺寸小一圈。
pub(super) const BOXES: [(f32, f32, f32, f32); """
        + str(len(boxes))
        + """] = [
"""
        + "\n".join(
            f"    ({num(b[0])}, {num(b[1])}, {num(b[2])}, {num(b[3])}), // {name}"
            for name, b in boxes
        )
        + """
];"""
    )

    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(header + "\n" + "\n\n".join(body) + "\n", encoding="utf-8")
    print(f"写好 {OUT.relative_to(HERE.parents[1])}")


if __name__ == "__main__":
    main()
