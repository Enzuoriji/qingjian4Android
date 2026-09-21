#!/usr/bin/env python3
"""把 Unicode 的 `emoji-test.txt` 转成表情面板要的那张表。

    python assets/emoji/render-panel.py

写出 `assets/emoji/emoji-panel.tsv`：一行一个 emoji，`分组\temoji\t中文名\t英文名`。

**为什么不用现成的 `emoji-zh.tsv`**：那张是「词 → emoji」，给候选用的（打「笑」出 😄）——
同一个 emoji 挂在好几个词下、顺序也不是面板要的。面板要的是「按分类排好、一屏一屏翻」，
而 `emoji-test.txt` 正是这么排的（文件里自己写着 "This file is in CLDR order…
recommended for keyboard palettes"）。

中文名从 `emoji-zh.tsv` **反查**：那个 emoji 挂在好几个词下时取最短的那个
（「😀」在 笑脸 / 露齿笑 / 开心 里取「笑脸」）。

原料在 `data/emoji/emoji-test.txt`（gitignore，用这条命令拿）：

    curl -sL -o data/emoji/emoji-test.txt https://unicode.org/Public/emoji/latest/emoji-test.txt
"""
import pathlib
import sys

HERE = pathlib.Path(__file__).resolve().parent
ROOT = HERE.parents[1]
SOURCE = ROOT / "data/emoji/emoji-test.txt"
FONT = HERE / "NotoColorEmoji.ttf"
NAMES = HERE / "emoji-zh.tsv"
OUT = HERE / "emoji-panel.tsv"

# 跳过的分组：Component 里全是肤色、发型这类「部件」，是给别的 emoji 拼着用的，
# 单独摆出来点一下没有意义（搜狗那几个面板也都不收）。
SKIP_GROUPS = {"Component"}

# 肤色修饰符 U+1F3FB..U+1F3FF。带肤色的变体不要：每个手势、每张脸都有五六种肤色，
# 全收进来「人物」那一组会占掉全表六成，翻起来没完（搜狗那几个面板也只收默认肤色）。
SKIN_TONES = set(range(0x1F3FB, 0x1F400))


def font_points():
    """随包那张字体里有字形的码点。

    **照它过滤**：字体比 Unicode 那张表旧，表里有、字体里没有的会画成一个豆腐块
    （截图里 `🫡` 那种）。按版本号猜（「Emoji 14 以后的不要」）不如直接问字体，
    反正这张表就是给它配的。

    要 `fontTools`（`pip install fonttools`）；没装就不过滤，只在终端吱一声。
    """
    try:
        from fontTools.ttLib import TTFont
    except ImportError:
        print("  （没装 fontTools，这一轮不按字体过滤）")
        return None
    font = TTFont(FONT, lazy=True)
    points = set()
    for table in font["cmap"].tables:
        points.update(table.cmap.keys())
    return points


def zh_names():
    """emoji → 中文名。一个 emoji 挂在好几个词下时取最短的那个词。"""
    names = {}
    for line in NAMES.read_text(encoding="utf-8").splitlines():
        if not line or line.startswith("#"):
            continue
        word, _, emojis = line.partition("\t")
        for emoji in emojis.split():
            best = names.get(emoji)
            # 短的更像个「名字」；一样长就留先来的（表里靠前的更常用）
            if best is None or len(word) < len(best):
                names[emoji] = word
    return names


def main():
    if not SOURCE.is_file():
        sys.exit(f"没有 {SOURCE.relative_to(ROOT)}，先按文件头那条 curl 下载")
    names = zh_names()
    points = font_points()
    skipped = 0

    group = ""
    rows = []
    for line in SOURCE.read_text(encoding="utf-8").splitlines():
        if line.startswith("# group:"):
            group = line.removeprefix("# group:").strip()
            continue
        if not line or line.startswith("#"):
            continue
        codes, _, rest = line.partition(";")
        status, _, comment = rest.partition("#")
        if status.strip() != "fully-qualified":
            # 只要完全限定的：其余是「缺变体选择符」的写法，同一个 emoji 会重复出现
            continue
        if group in SKIP_GROUPS:
            continue
        if any(int(code, 16) in SKIN_TONES for code in codes.split()):
            continue
        # 注释形如 `😀 E1.0 grinning face`：第一个空格前是字符，版本号之后是英文名
        parts = comment.strip().split(" ", 2)
        if len(parts) < 3:
            continue
        emoji, english = parts[0], parts[2]
        if points is not None and not all(ord(c) in points for c in emoji):
            skipped += 1
            continue
        rows.append((group, emoji, names.get(emoji, english), english))

    if not rows:
        sys.exit("一个 emoji 都没解析出来，emoji-test.txt 的格式是不是变了")

    text = [
        "# 表情面板的排布：分组\\temoji\\t中文名\\t英文名，按 Unicode 的 CLDR 顺序（面板就照这个排）\n",
        "# 由 render-panel.py 从 Unicode emoji-test.txt 生成（Unicode License v3），别手改\n",
    ]
    last = None
    for group, emoji, zh, en in rows:
        if group != last:
            text.append(f"# group: {group}\n")
            last = group
        text.append(f"{group}\t{emoji}\t{zh}\t{en}\n")
    OUT.write_text("".join(text), encoding="utf-8")

    groups = sorted({row[0] for row in rows})
    print(
        f"写好 {OUT.relative_to(ROOT)}：{len(rows)} 个 emoji、{len(groups)} 个分组"
        + (f"（滤掉 {skipped} 个字体里没有的）" if skipped else "")
    )
    for name in groups:
        count = sum(1 for row in rows if row[0] == name)
        print(f"  {name}: {count}")


if __name__ == "__main__":
    main()
