#!/usr/bin/env python3
"""把 `menu.svg` 那条路径转成 Rust 的 `PathBuilder` 调用。

渲染器里那个小标（候选条没组句时那条细的左边）用的就是它——**画路径不画字形**，
理由与 `gear.rs` 一样：`U+1F4DC` 这类字符会落进 emoji 字体或者缺字，画出来不可控。

    python assets/icon/render-logo-path.py

写出 `crates/qingjian-render/src/logo/path.rs`，生成物**随仓库提交**（与 Windows 那几档
任务栏图标一个做法）。改了 svg 就重跑这条命令，别手改生成物。

坐标保持 svg 自己的 viewBox（39×28），缩放交给 `logo::draw_logo`。
"""
import pathlib
import re
import sys

HERE = pathlib.Path(__file__).resolve().parent
OUT = HERE.parents[1] / "crates/qingjian-render/src/logo/path.rs"

# 每个命令有几个参数（一段一组）
ARITY = {"M": 2, "L": 2, "H": 1, "V": 1, "C": 6, "Z": 0}

TOKEN = re.compile(r"[MmLlHhVvCcZz]|-?\d*\.?\d+(?:e-?\d+)?")


def tokens(d):
    return TOKEN.findall(d)


def parse(d):
    """切成一段段子路径：每段是 [(命令, [参数...]), ...]。"""
    ts = tokens(d)
    paths, current = [], None
    x = y = 0.0
    i = 0
    command = None
    while i < len(ts):
        if ts[i].isalpha():
            command = ts[i]
            i += 1
            if command in "Mm":
                current = []
                paths.append(current)
        if command is None:
            raise ValueError("第一个记号就该是命令")
        upper = command.upper()
        if upper == "Z":
            current.append(("close", []))
            command = None
            continue
        n = ARITY[upper]
        args = [float(v) for v in ts[i : i + n]]
        if len(args) != n:
            raise ValueError(f"{command} 参数不够")
        i += n
        relative = command.islower()
        if upper == "M":
            x, y = (x + args[0], y + args[1]) if relative else tuple(args)
            current.append(("move_to", [x, y]))
            # moveto 之后跟着的坐标对是隐式的 lineto
            command = "l" if relative else "L"
            continue
        if upper == "H":
            x = x + args[0] if relative else args[0]
            current.append(("line_to", [x, y]))
            continue
        if upper == "V":
            y = y + args[0] if relative else args[0]
            current.append(("line_to", [x, y]))
            continue
        if upper == "L":
            x, y = (x + args[0], y + args[1]) if relative else tuple(args)
            current.append(("line_to", [x, y]))
            continue
        if upper == "C":
            pts = [
                (x + args[0], y + args[1]),
                (x + args[2], y + args[3]),
                (x + args[4], y + args[5]),
            ] if relative else [
                (args[0], args[1]),
                (args[2], args[3]),
                (args[4], args[5]),
            ]
            x, y = pts[2]
            current.append(("cubic_to", [c for p in pts for c in p]))
            continue
        raise ValueError(f"这条命令没实现：{command}")
    return paths


def num(v):
    text = f"{v:g}"
    return text if "." in text or "e" in text else text + ".0"


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
    svg = (HERE / "menu.svg").read_text(encoding="utf-8")
    match = re.search(r'\sd="([^"]+)"', svg)
    if not match:
        sys.exit("menu.svg 里没找到 path 的 d")
    paths = parse(match.group(1))
    if len(paths) != 5:
        sys.exit(f"该是一枚键帽 + 四片竹简（5 段子路径），实际 {len(paths)} 段")

    header = '''//! 青简那个标的两部分路径：**一枚键帽 + 四片竹简**（竹简是挖空的孔）。
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
'''
    body = [
        emit_function(
            "keycap",
            "键帽的外形（39×28 的圆角矩形）。",
            paths[0],
        )
    ]
    for index, slip in enumerate(paths[1:]):
        body.append(
            emit_function(
                f"slip{index}",
                f"第 {index + 1} 片竹简——**挖空**用的（见 `draw_logo`）。",
                slip,
            )
        )
    body.append(
        """/// 四片竹简，`slip0` 起。
pub(super) const SLIPS: [fn() -> Option<Path>; 4] = [slip0, slip1, slip2, slip3];"""
    )

    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(header + "\n" + "\n\n".join(body) + "\n", encoding="utf-8")
    print(f"写好 {OUT.relative_to(HERE.parents[1])}")


if __name__ == "__main__":
    main()
