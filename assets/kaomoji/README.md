# 颜文字

## panel.tsv

表情面板「颜文字」那一页的排布：一行一条，`分类\t颜文字`，分组用 `# 分类` 注释标出。

- 来源：[kaomojiya-collection/kaomoji-collection](https://github.com/kaomojiya-collection/kaomoji-collection)
  （MIT，版权行见同目录 `LICENSE`；那个仓自己说明是从 <https://www.kaomojiya.org> 整理的）
- **不是全量**：原表 535 个分类、4.1 万条，这里只挑了 **22 个分类、每类前 40 条**。
  理由见 `render-panel.py` 的头注释——535 个分类大半是日文罗马字（`yorokobu`、`chokon`），
  当不了面板标签；4 万条也翻不完。原表按长度排，前面那些正是最常用的。
- 生成：

  ```sh
  curl -sL -o data/kaomoji/kaomoji.json \
    https://raw.githubusercontent.com/kaomojiya-collection/kaomoji-collection/main/kaomoji.json
  python assets/kaomoji/render-panel.py
  ```

  原始 JSON 放 `data/`（不进仓库），生成物 `panel.tsv` 随仓库提交。
  换分类、改每类收几条，都改脚本里的 `CATEGORIES` / `PER_CATEGORY` 再重跑。

## 为什么挑着收，而不是全量

一是**面板上放不下**：搜狗那种面板一屏 20 来格，535 个分类得翻到什么时候。
二是**挑出来的质量更整齐**：原表按长度排，短的正是最常用的（`☺︎`、`(*◡̈)`、`ฅ•ω•ฅ`），
长的那些是「画一整幅画」的，塞进格子里还得补省略号，反而认不出来（脚本里 `MAX_CHARS` 挡掉）。
