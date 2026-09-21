# 图标

- `logo.png`（866×866，带透明通道）：应用图标源文件。`apps/macos/scripts/bundle.sh` 打包时用 `sips` + `iconutil`
  生成 `Qingjian.icns`，生成物不进仓库。
- `menu.svg`：macOS 输入法图标源文件，黑色键帽镂空四片竹简（模板图，系统只取 alpha）。`menu.pdf` 是它导出的
  22×16pt 矢量版，打包时拷成 `qingjian-menu.pdf`，Info.plist 的图标键都指向它。为什么是这个形式和尺寸见
  `docs/design/architecture.md`「Info.plist 约定」。改了 svg 重新导出：

  ```sh
  rsvg-convert -f pdf --page-width 22pt --page-height 16pt -w 22pt -h 16pt assets/icon/menu.svg -o assets/icon/menu.pdf
  ```

  同一个形**安卓那边也在用**：候选条没组句时那条细的最左边的标（点开工具页 / 剪贴板）。
  渲染器不解析 svg，所以那条路径由 `render-logo-path.py` 转成 Rust 代码：

  ```sh
  python assets/icon/render-logo-path.py
  ```

  写出 `crates/qingjian-render/src/logo/path.rs`（**生成物随仓库提交**），画法在 `logo/mod.rs`。
  改了这个 svg，这两处都要重跑。
- `material/`：**键盘上那几个图标**的源文件——⇧ 大小写（`keyboard_capslock.svg`）、
  ⌫ 退格（`backspace.svg`）、剪贴板（`content_paste.svg`），来自 Google 的
  [Material Symbols](https://fonts.google.com/icons)（Apache-2.0，全文见 `LICENSE-material-symbols.txt`）。
  下载地址形如 `https://fonts.gstatic.com/s/i/short-term/release/materialsymbolsoutlined/<名字>/default/24px.svg`
  （`<名字>` 就是不带扩展名的文件名），取的是 **outlined 家族的默认档**——安卓上到处见的就是这一套。

  2026-09-21 之前这几个图标是**手写的坐标**，用户看了说「不要这样做去网上找可以用的」，换成了这个。

  渲染器不解析 svg，路径由 `render-key-icon-path.py` 转成 Rust 代码：

  ```sh
  python assets/icon/render-key-icon-path.py
  ```

  写出 `crates/qingjian-render/src/renderer/keyboard/icon/path.rs`（**生成物随仓库提交**），
  画法在 `renderer/keyboard/icon.rs`。改了这里那几张 svg、或者要加新图标（在脚本的 `ICONS` 里加一行）
  就重跑那条命令。解析 svg 路径的小工具是 `svgpath.py`，跟 `render-logo-path.py` 共用一份。

- `windows/mode-zh.svg` / `mode-en.svg` / `mode-caps.svg`：Windows 任务栏的中 / 英 / A 图标源文件（16×16 画布，单色）。
  `windows/render-mode-icons.sh` 用 rsvg-convert + magick 栅格化成 16 / 20 / 24 / 32 四档的 8 位 alpha 蒙版，
  写到 `apps/windows/tsf/resources/mode/`，DLL 用 `include_bytes!` 嵌入、运行时按任务栏深浅色填色（`com/mode/icon.rs`）。
  改了 svg 重跑脚本，生成物随仓库提交。
