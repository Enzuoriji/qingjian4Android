"""把 SVG 里那条 `d` 解析成一段段子路径——几个生成脚本共用这一份。

只认 `M L H V C Q T Z` 这几条（含相对命令、隐式重复、以及 `Z` 之后当前点回到子路径起点）。
够用了：`menu.svg` 与 Material Symbols 那套图标用的就是这些。
**圆弧 `A` 没实现**——哪天真碰上再说，那时候记得按椭圆参数化成贝塞尔。

    from svgpath import parse, bbox_of, d_of
"""
import re
import sys

# 每个命令有几个参数（一个记号一组）
ARITY = {"M": 2, "L": 2, "H": 1, "V": 1, "C": 6, "Q": 4, "T": 2, "Z": 0}

TOKEN = re.compile(r"[MmLlHhVvCcQqTtZz]|-?\d*\.?\d+(?:e-?\d+)?")


def d_of(svg_text):
    """从 svg 文本里抠出 `d` 属性的值（取第一条 `path`）。"""
    match = re.search(r'\sd="([^"]+)"', svg_text)
    if not match:
        sys.exit("这个 svg 里没找到 path 的 d")
    return match.group(1)


def parse(d):
    """切成一段段子路径：每段是 `[(命令, [参数...]), ...]`。

    命令名是 `PathBuilder` 那套：`move_to` / `line_to` / `cubic_to` / `quad_to` / `close`。
    `quad_to` 的参数是**绝对坐标**（`T` 的反射控制点也在这儿算好），
    与先前的 `cubic_to` 一致——缩放的活儿交给 `Path::transform`，生成的就是原始坐标。
    """
    ts = TOKEN.findall(d)
    paths, current = [], None
    x = y = 0.0
    # 子路径的起点：`Z` 之后当前点要回到这儿（后面跟的相对命令是相对它的）
    start = (0.0, 0.0)
    # 上一个二次贝塞尔的控制点，`T` 要靠它反射；没跟过 `Q`/`T` 时反射点就是当前点
    control = None
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
            x, y = start
            control = None
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
            start = (x, y)
            current.append(("move_to", [x, y]))
            # moveto 之后跟着的坐标对是隐式的 lineto
            command = "l" if relative else "L"
            control = None
            continue
        if upper == "H":
            x = x + args[0] if relative else args[0]
            current.append(("line_to", [x, y]))
            control = None
            continue
        if upper == "V":
            y = y + args[0] if relative else args[0]
            current.append(("line_to", [x, y]))
            control = None
            continue
        if upper == "L":
            x, y = (x + args[0], y + args[1]) if relative else tuple(args)
            current.append(("line_to", [x, y]))
            control = None
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
            control = pts[1]
            continue
        if upper == "Q":
            (cx, cy) = (x + args[0], y + args[1]) if relative else (args[0], args[1])
            x, y = (x + args[2], y + args[3]) if relative else (args[2], args[3])
            current.append(("quad_to", [cx, cy, x, y]))
            control = (cx, cy)
            continue
        if upper == "T":
            # 平滑二次：控制点是**上一个控制点关于当前点的反射**；没跟过就是当前点本身
            (cx, cy) = (2 * x - control[0], 2 * y - control[1]) if control else (x, y)
            x, y = (x + args[0], y + args[1]) if relative else tuple(args)
            current.append(("quad_to", [cx, cy, x, y]))
            control = (cx, cy)
            continue
        raise ValueError(f"这条命令没实现：{command}")
    return paths


def num(v):
    """写成 Rust 认的字面量（`3` → `3.0`）。"""
    text = f"{v:g}"
    return text if "." in text or "e" in text else text + ".0"


def bbox_of(paths, steps=24):
    """这几段子路径的包围盒（**按曲线真正走到的点算**，不是照控制点——控制点会外扩）。"""
    xs, ys = [], []

    def walk(x0, y0, segments):
        x, y = x0, y0
        for cmd, args in segments:
            if cmd == "move_to":
                x, y = args
                xs.append(x)
                ys.append(y)
            elif cmd == "line_to":
                x, y = args
                xs.append(x)
                ys.append(y)
            elif cmd in ("cubic_to", "quad_to"):
                # 三次与二次各按自己的式子采样
                if cmd == "cubic_to":
                    (ax, ay, bx, by, cx, cy) = args
                    points = [
                        (
                            (1 - t) ** 3 * x
                            + 3 * (1 - t) ** 2 * t * ax
                            + 3 * (1 - t) * t**2 * bx
                            + t**3 * cx,
                            (1 - t) ** 3 * y
                            + 3 * (1 - t) ** 2 * t * ay
                            + 3 * (1 - t) * t**2 * by
                            + t**3 * cy,
                        )
                        for t in (i / steps for i in range(steps + 1))
                    ]
                else:
                    (ax, ay, cx, cy) = args
                    points = [
                        (
                            (1 - t) ** 2 * x + 2 * (1 - t) * t * ax + t**2 * cx,
                            (1 - t) ** 2 * y + 2 * (1 - t) * t * ay + t**2 * cy,
                        )
                        for t in (i / steps for i in range(steps + 1))
                    ]
                xs.extend(p[0] for p in points)
                ys.extend(p[1] for p in points)
                x, y = points[-1]

    for segments in paths:
        walk(0.0, 0.0, segments)
    return min(xs), min(ys), max(xs), max(ys)
