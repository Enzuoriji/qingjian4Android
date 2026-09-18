# 表情数据

## emoji-zh.tsv / emoji-en.tsv

Unicode CLDR 的 annotations（中文 / 英文）转出的 emoji 表：中文词与英文词各配 emoji，加载时合成一张。
许可见 `LICENSE-unicode.txt`（Unicode License v3，可发布）。生成命令见 `docs/notes/crate-notes.md` 的 assets 一节。

## NotoColorEmoji.ttf

**为什么随包带字体**：安卓 15 起系统自带的 `NotoColorEmoji.ttf` 换成了纯 COLR v1 格式，
一条 v0 兼容记录都没留；而渲染器用的 swash 只读 v0 记录（`swash/src/scale/color.rs` 的 `layers()`），
于是 emoji 会画成空白（安卓的 `Segoe` 对应物 —— Windows 的 `Segoe UI Emoji` 同样是 v1，但留了 3365 条
v0 记录，所以能画）。这里随包带一份**位图格式（CBDT/CBLC）**的 NotoColorEmoji，渲染器走彩色位图那条路，
与系统字体同款设计、同款字形。

- 来源：<https://github.com/googlefonts/noto-emoji> 的 `fonts/NotoColorEmoji.ttf`，标签 `v2.051`
- 格式：CBDT/CBLC 位图（不是 COLR），单 strike，ppem 137
- 大小：10.7 MB（AGP 打包时会压，进 APK 约 7 MB；解到应用私有目录后占 10.7 MB）
- 许可：SIL Open Font License 1.1，见 `LICENSE-noto-emoji.txt`
- 升级注意：**别换成 `Noto-COLRv1.ttf`**，那个 swash 画不了；换版本后用
  `cargo run -p qingjian-render --example emoji_probe -- <字体文件>` 确认输出是 `Color N×N` 而不是 `Mask 0×0`

文件放在这里（而不是 `apps/android/app/src/main/assets/`）是为了跟 emoji 表放一起、来源与许可写在一处；
安卓的 `app/build.gradle.kts` 把这个目录整个挂成 assets，所以它跟着 APK 走。
