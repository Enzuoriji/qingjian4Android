#!/usr/bin/env python3
"""把 `kaomoji-collection` 那份表转成颜文字面板要的形状。

    python assets/kaomoji/render-panel.py

写出 `assets/kaomoji/panel.tsv`：`分类\t颜文字`，按 [`CATEGORIES`] 里的顺序。

**为什么要挑**：原始表有 535 个分类，大半是日文罗马字（`yorokobu` 喜ぶ、`sumurai`
スマイル、`chokon` チョコン…），面板上没法当标签用；4 万条也翻不完。这里只挑
含义明确、日常真会用的那十几个英文分类，每类取前 [`PER_CATEGORY`] 条——原表**按长度
从短到长排**，短的正是最常用的（`☺︎`、`(*◡̈)`），取前面一段正合适。

原料在 `data/kaomoji/kaomoji.json`（gitignore）：

    curl -sL -o data/kaomoji/kaomoji.json \\
      https://raw.githubusercontent.com/kaomojiya-collection/kaomoji-collection/main/kaomoji.json
"""
import json
import pathlib
import sys

HERE = pathlib.Path(__file__).resolve().parent
ROOT = HERE.parents[1]
SOURCE = ROOT / "data/kaomoji/kaomoji.json"
OUT = HERE / "panel.tsv"

# 面板上的分类：`(原表里的分类名, 面板上写的中文标签)`。
#
# 挑的都是英文名、含义一看就懂的；日文罗马字那些（`yorokobu`、`doya`）不收——
# 一个是没法当标签，一个是那些细分的情绪面板上分不过来。
CATEGORIES = [
    ("smile", "开心"),
    ("happy", "高兴"),
    ("love", "爱心"),
    ("kiss", "亲亲"),
    ("hug", "抱抱"),
    ("wink", "眨眼"),
    ("cry", "哭"),
    ("sad", "难过"),
    ("angry", "生气"),
    ("scary", "害怕"),
    ("surprise", "惊讶"),
    ("shy", "害羞"),
    ("tired", "累了"),
    ("cat", "猫"),
    ("dog", "狗"),
    ("rabbit", "兔子"),
    ("thanks", "谢谢"),
    ("sorry", "抱歉"),
    ("applause", "鼓掌"),
    ("dance", "跳舞"),
    ("salute", "敬礼"),
    ("peace", "耶"),
]

# 每个分类收多少条。原表按长度排，前面这些是最短也最常用的。
PER_CATEGORY = 40

# 一条颜文字最多几个字符。太长的（那种画一整幅画的）在这个格子里放不下，
# 补省略号又看不出是什么，干脆不收。
MAX_CHARS = 24


def main():
    if not SOURCE.is_file():
        sys.exit(f"没有 {SOURCE.relative_to(ROOT)}，先按文件头那条 curl 下载")
    table = json.loads(SOURCE.read_text(encoding="utf-8"))

    rows = []
    for name, label in CATEGORIES:
        items = table.get(name)
        if not items:
            print(f"  （原表里没有 {name}，跳过）")
            continue
        seen = set()
        for item in items:
            text = item.strip()
            if not text or text in seen or len(text) > MAX_CHARS:
                continue
            seen.add(text)
            rows.append((label, text))
            if len(seen) >= PER_CATEGORY:
                break

    if not rows:
        sys.exit("一条颜文字都没挑出来，原表的结构是不是变了")

    text = [
        "# 颜文字面板的排布：分类\\t颜文字\n",
        "# 由 render-panel.py 从 kaomoji-collection 挑出来（MIT，见同目录 LICENSE），别手改\n",
    ]
    last = None
    for label, kaomoji in rows:
        if label != last:
            text.append(f"# {label}\n")
            last = label
        text.append(f"{label}\t{kaomoji}\n")
    OUT.write_text("".join(text), encoding="utf-8")

    labels = [label for label, _ in rows]
    print(f"写好 {OUT.relative_to(ROOT)}：{len(rows)} 条、{len(set(labels))} 个分类")
    print("  " + " · ".join(f"{name}" for name in dict.fromkeys(labels)))


if __name__ == "__main__":
    main()
