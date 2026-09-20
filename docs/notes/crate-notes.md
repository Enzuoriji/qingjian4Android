# 各 crate 的实现要点

CLAUDE.md 只保留目录地图与规则，每个 crate / app / tool 的实现细节收在这里：入口类型、数据文件、常数、生成命令。
改了实现要同步改这里；与代码冲突时以代码为准。

## crates/qingjian-dictionary

词库（TSV 解析或 `.qj` mmap），键按字节序排好，查询逐音节位置二分收窄（简拼位置按音节块跳扫），
`lookup_pattern`（≥ 模式长度）与 `lookup_exact`（正好等长）同一套实现。词库键以 `v` 表示 ü，
TSV 解析、查询与生成工具把 `lue` / `nue` 统一成 `lve` / `nve`。
旧 `.qj` 含这些键时，加载器建立规范化的内存词库。新 `.qj` 继续使用 mmap。

## crates/qingjian-core

模块：`composition` / `parser` / `correction`（拼写纠错：整段一处编辑的候选纠正 + `typo` 音节级敲错变体表，后者进整句词图当带代价的边）/
`candidate` / `ranking` / `shortcut` / `sentence` / `fuzzy` / `shuangpin`（双拼：四套方案键位表、键 → 全拼解码与消耗换算）/ `zhuyin`（大千注音：键 → 注音符号 → 拼音，`[general] zhuyin` 开关，声调只判音节完整不进查询）/ `emoji` /
`english`（英文模式候选）/ `engine`（`query::EnglishTail`：句末英文词并入整句，`woxiangxuehaorust` → 我想学好rust，尾段也像拼音时按分数与拼音读法比）。
`Engine` 是对外唯一门面，`Translator` / `Learner` trait 在 `engine` 模块；词库是「主词库 + 附加词库（`set_extra_dictionaries`）+ 用户词」的列表；繁体输出（`traditional` 开关与 `traditional_map` 映射）依赖 `ferrous-opencc`（`s2tw`）在出候选与上屏边界转换，内部保持简体。
`Engine` 是对外唯一门面，`Translator` / `Learner` trait 在 `engine` 模块；词库是「主词库 + 附加词库（`set_extra_dictionaries`）+ 用户词」的列表。
- 中英混输的英文词位置：`Engine::set_chinese_first`（配置 `[general] chinese_first`，缺省关）关着时拼音不像话的输入英文排第一（`extras::insert_english`，
  用户老选中文词时仍让中文在前），开着时整句先插、英文词紧随其后排第二（`query_inner` 里两步的先后按开关掉转）；句末英文词并入整句（`EnglishTail`）不受它影响。
  缺省关是回放定的（9241 词 / 269 条英文上屏：缺省开英文首选 82.5% → 7.1%）。
- `custom_phrase::merge_replacements` 把平台给的「输入码 → 短语」表（macOS 系统文本替换）并进配置里的自定义短语：每条占该码最靠前的空位（1–9），
  输入码不是小写字母、已有同码同文本、九位都满的跳过；Core 不管数据从哪来。

`EngineSession` 保存可挂起的组句、标点、历史与学习链，`Engine::swap_session` 在同一个引擎里交换输入状态，共用词库与落盘服务。切换上下文时清除查询及异步预测缓存，并由平台恢复各自私密状态。

`Engine::discard_input` / `EngineSession::discard_input` 用于隐私能力变化时无痕清理输入，包括透传缓冲、学习链和暂存词汇曝光；`set_private` 只切换写入开关，保留已输入的组句。

## crates/qingjian-translate

`Glossary`，本地 TSV 释义表（词性 + 译文）；`LevelTable`，词汇等级表（`assets/levels/levels-{en,ja}.tsv`，CEFR A1–C2 / JLPT N5–N1，
`uv run tools/corpus/levels.py` 从 `data/levels/` 的原始 CSV 生成，来源与许可见 `assets/levels/README.md`），「统计」页按级数词汇用，不进候选。

## crates/qingjian-learning

- `FrequencyLearner`：用户选择次数（`user.tsv`）、按输入串记的选择（`user-choices.tsv`，词级排序里同输入串选过的优先）、用户词（`user-words.tsv`，主词库同格式，
  Engine 与主词库一起查）、个人英文词（`user-english.tsv`，回车原样上屏的英文词与选过的英文候选，与随包英文词表一起出候选且在前）、
  个人敲错表（`user-typos.tsv`，接受过的 (敲的, 要的) 音节对，词图敲错边与整段纠错的代价按它打折）与个人 n-gram（`user-ngram.tsv`，Core `sentence::UserNgram`，
  二元 + 三元在线计数，整句转换与词级排序里与静态模型插值；Tab 接受的云端整句按 `sentence::segment_text` 切词后也记；
  连着选出的两个词记够次数自动造词进用户词，一段拼音分几次选完的合成词记两次也造）。
- `InputLog`：输入日志（`input-log.jsonl`，每次上屏一行：敲的键、切分、看到的前几个候选、选了第几个、来源、纠错、撤销，
  Core `InputLogger` trait 的落盘实现，`[general] input_log` 缺省开，只写本机，给离线回归评测与个人模型用）。
- `UsageStats`：输入统计（`usage.tsv`，按天记汉字 / 中文词 / 英文词 / 上屏次数，Core `UsageMeter` trait 的实现，Engine 每次上屏 `Usage::of_text` + 按来源定词数，
  整句按 `segment_text` 切词数；与输入日志无关，偏好设置「统计」页显示，`book_scale` 折成几本《某书》）。
- `VocabularyBook`：词汇记录（`user-vocab.tsv`，Core `VocabularyTracker` trait 的实现：学习语言的每条译词看到过几轮 / 上屏过 / ⌥+数字 打出过几次；Core 私密输入统一跳过曝光和提交写入，但仍可读取已有记录用于排序和生词标记；
  Engine `annotate` 据此填 `Sense::fresh`，看到轮次不到 `FRESH_UNTIL` = 3 的译词壳里画橙色；「看到」按上屏那一刻屏幕上那一页算，壳每次画完 `Engine::note_displayed` 告知当前页）。
- 各表落盘走 Core `storage::write_atomic`（临时文件 + fsync + 改名），加载按行容错（坏行警告跳过，真读不了壳退回内存学习），
  壳激活期间每 60 秒 `Engine::flush_learning`；IMK 回调边界 `imk::catch_panic` 拦 panic、缓冲区字母原样上屏（见 architecture.md「崩溃不丢」）。

## crates/qingjian-predict

- `CloudPredictor`：`Predictor` trait 的网络实现（async-openai，OpenAI 兼容接口，默认 DeepSeek），后台线程防抖 / 缓存 / 超时，`submit` / `poll` 非阻塞。
  `PredictConfig` 是配置的 `[predict]` 分节。只在组句中联想，一次请求给云端词（容错校验后补进候选第一页末尾 `[predict] slots` 格，缺省 2，不预留不占位，
  前面的本地候选不挪；排布在 Core `CandidateLayout`）和整句补全（preedit 右侧，Tab）；上屏后不联想，本地历史不进请求。
- `CloudGlossFiller`：释义兜底（Core `GlossFiller` trait，与 Predictor 分开的线程与通道，攒 1.5 秒 / 8 个词发一次，问过不再问）：
  随包释义表没有的词库词 / 云端词上屏后入队，结果壳每秒 `Engine::poll_glosses` 经 `Translator::learn` 写进 `qingjian-translate::PersonalGlossary`
  （`user-glossary-<语言>.tsv`，`LayeredTranslator` 个人表优先）；随云联想开关一起开。
- 问字键（缺省 `u`）开头是问字模式（`PredictionKind::Question`，答案带读音、不校验拼音），`?` 开头要 `ModeKeys::question_mark` 开着才算（配置 `[shortcut] question_mark`，缺省关，壳用 `Engine::takes_question_mark` 决定空缓冲区的 `?` 是入口还是标点）；`PredictionKind::Translate` 是壳里快捷键触发的「翻译选中文字」
  （双向：汉字为主译成学习语言，外文译回中文，`prediction::translation_target`），译文走结果的 `sentence`。

## crates/qingjian-format

`.qj` 数据容器（`Container` mmap 读、`Writer` 写、`Table<T>` / `Text` 零拷贝视图、`hash` 可落盘哈希索引、`Metadata` 名称 / 许可证 / 署名）。
词库与语言模型都能 `write_qj` / 从 `.qj` 打开，启动 50 ms；`cargo run --release -p qingjian-dict-convert -- pack dict|lm --name … --license …`
生成 `data/generated/{dict,lm}.qj`，`bundle.sh` 在 TSV 更新时自动重打并只把 `.qj` 打进包。设计见 `docs/design/architecture.md`「数据文件：`.qj` 容器」。

## crates/qingjian-neural

`CharScorer`，Core `sentence::SentenceScorer` trait 的实现：candle 加载字级 Transformer（GPT-2 风格 decoder，训练仓库（本地 `../train`，私有，不在本仓库）导出的
`model.safetensors` + `config.json` + `vocab.json`），给「前文 + 整句」按字累加 log 概率；前文的每层 K / V 缓存（`PrefixCache`），
同一段前文只算一次，每个候选只算自己那几个字（64 字前文 × 8 条 28 ms，Metal）。features `accelerate` / `metal` 换后端，壳用 `metal`。

Engine 侧在 `engine/rescoring/`：接了打分器就取 Viterbi 前 `RESCORE_PATHS` = 6 条路径按 `路径分 + λ·(神经分 − 静态二元分)` 重排（λ `NEURAL_WEIGHT` 0.5，
个人 n-gram / 用户加分 / 代价不动），分走「前文 + 文本 → 神经分」缓存 `NeuralCache`；同步打分器（`with_sentence_scorer`，CLI 评测）当场补分，
异步的（`with_async_sentence_scorer`，后台线程 `RescoreWorker`）查询不等模型：缺分的记下来，壳停键后 `request_rescoring`、`poll_rescoring` 到了再 `query` 一次。
前文优先用壳给的应用光标前文（`set_rescoring_context`），没有用本会话最近 64 个上屏字符。CLI `--neural <导出目录>`（`--neural-weight` / `--neural-context` / `--neural-async`）。

## crates/qingjian-lm

`BigramModel`，Core `sentence::LanguageModel` trait 的实现，从 `data/generated/lm.qj`（或 `lm-unigram.tsv` / `lm-bigram.tsv`）加载
（没有这两个文件就退化为一元词频整句）。数据由 `tools/corpus/parquet_to_text.py`（uv 脚本，HF parquet → 简体纯文本）加
`cargo run --release -p qingjian-dict-convert -- bigram --phrases assets/lexicon/phrases.tsv --phrases assets/lexicon/domain_words.tsv --brand assets/lexicon/brand.tsv --brand assets/lexicon/mixed_words.tsv data/corpus/*.txt` 生成；语料在 `data/corpus/`（gitignore）。
短语层不当 token 统计（分词时摘掉、统计完按成分合成一元 / 二元，短语得分等于原来两个词的路径，见 `bigram.rs` 模块注释），品牌词按给定次数写进一元与句首二元。

## crates/qingjian-platform

`Config`（TOML 配置文件，`[general]` / `[shortcut]` / `[fuzzy]` / `[dictionaries]` / `[apps]` / `[predict]` 分节，首次运行写模板，
`set_value` 用 toml_edit 原地改键保留注释；`[model] enabled` 本地整句模型开关，`LocalModelConfig`）；`extra_dictionaries` 列出 / 加载随包领域词库与用户 `dicts/`
（mac 壳与 Windows Server 共用，同名 `.qj` 优先于 `.tsv`）；`protocol` 模块是 Windows Server ↔ TSF DLL 的 IPC 协议类型
（`ClientMessage` / `ServerMessage` / `Frame` / `PreeditSegment`，全 serde，两端共用，见 `docs/design/architecture.md`「Windows：TSF」）。

## crates/qingjian-render

自绘渲染器：候选窗一帧 + 主题 → 预乘 RGBA 位图，tiny-skia 栅格 + cosmic-text 文字（fontdb 按平台清单只加载几个字体文件、不扫系统），
自己解析 `trak` 字距表、按主题 gamma 加深笔画；cosmic-text 打了 `opsz` 光学字号补丁（qingjian-team/cosmic-text 分支 `qingjian-opsz`，workspace `[patch.crates-io]` 钉 rev）。
`examples/preview.rs` 出 PNG 与真机截图并排比、`--measure` 与 AppKit 对宽度。mac 壳 `candidates/bitmap/` 贴位图，`[general] renderer = "system"` 切回 AppKit 绘制
（过渡期退路，偏好设置「候选窗口」页可选）；`[general] font` 是候选窗字族名（空为系统字体，`bitmap/font_files.rs` 用 CoreText 按字族名找文件只加载那几个，没装就回系统字体；
设置页 `preferences/font_picker/` 是搜索框 + 列表）。设计与验收见 `docs/design/rendering.md`。

软键盘走同一条路：布局数据在 `keyboard/`（`KeyboardLayout` 按「单位宽」算几何，每行居中，第 2 行自然得到半键错位，不写死坐标）、绘制在 `renderer/keyboard/`、
主题在 `theme/keyboard.rs`；`render_keyboard` 与 `render_status` 一样，出位图的同时把**每个键的命中矩形**一并返回。设计与取舍见 `docs/design/keyboard.md`。

**横屏**（2026-09-20）：`KeyboardTheme::landscape()` 只把高度改成 176 点
（fcitx5 的横屏缺省是屏幕高的 49%，手机上横屏高 360 点就是 176；用固定点数是因为
平板横屏按百分比会给出巨无霸键盘）。横竖屏由壳判断后随 `configure` 一起报进来。

**壳报的宽度要用「视图量出来的」，不是屏幕宽**：输入法窗口不一定占满屏幕——
横屏时系统给挖孔 / 手势区让位，实测 720×1280 的机器横过来窗口只从 x=136 起、宽 1144。
照屏幕宽画会宽出窗口、右边一列键被切掉。竖屏两者相等，所以只在横屏露出来。
（视图还没量出来时退回屏幕宽，量出来之后 `onSizeChanged` 会再配一次。）

**键上图标（⇧ / ⌫）的尺寸要按点写、画的时候乘密度**。踩过的坑：边长的上限原先按**像素**写死，
而键高是像素值、随密度一起涨，于是**屏幕密度越高的手机图标相对越小**——真机上「退格 / 上档图标偏小」
就是这么来的，模拟器（密度 2）上却看着正好。现在 `SIZE_RATIO` 占键高 0.46（实机截图反推：
图标高占键高 0.33，而画法里图标只占方块的 0.72），上下限也按点算。
`renderer/keyboard` 的 `the_icons_scale_with_density` 盯着密度 2 与 3 下比例一致。

**三页键盘（2026-09-18）**：`keyboard/panel.rs` 是 `Panel`（字母 / 数字 / 符号），`KeyboardLayout::of(panel)` 取某一页的布局，
`KeyId::Literal(char)` 是「按一下出这个字符」，`KeyId::Panel(Panel)` 是切页键（标签写的是**要去哪一页**）。
**三页都是四行**——键盘高度是定死的，页与页行数不一样就会把上面的应用顶一下。
数字页按计算器那样排（`+ - * /` 一竖条，1-2-3 / 4-5-6 / 7-8-9，0 与切页键在末行），每行 5 个单位，
所以那一页的键比字母页宽一倍。

`KeyWidth` 有两种：`Units(f32)` 按单位宽，`Fill` **撑满这一行剩下的**（一行可以有多个，平分）。
`Fill` 那个键不参与 `unit_width` 的定宽（算 0），否则会跟自己的宽度循环论证；
有它的行**不居中、铺满整宽**，没有的才按总宽居中（第 2 行由此得到半键错位）。
字母页有两处用它对齐：第 3 行的 `⇧` 与 `⌫`（9 个键、8 条缝，比第 1 行少一条）、
最下一排的空格（7 个键、6 条缝，少三条）。第 2 行**故意不齐**——半键错位是 QWERTY 的样子。
2026-09-20 同一天还改了两处**位置**：逗号挪到空格左边、`Mode` 挪到句号与回车之间。
这块来回错过四次，现在三条测试盯着（`layout.rs` 一条盯结构、`renderer/keyboard` 两条按像素量），
别再凭感觉调。
字母键以外的键帽画的是**半角原字符**：全角与否由 `qingjian-core` 的标点表在打出去的时候转（`?`→`？`），
画死成固定全角在英文模式下就骗人了。**`Comma` 与 `Period` 是按模式画**——字母页底下这两个最常用，
中文画全角（`，`/`。`）一眼认得，英文画半角（`,`/`.`），交出去的**永远是半角**。

安卓的候选条是 `renderer/bar/`（`render_bar`）：通栏一条，上排拼音直接复用候选窗的 `draw_top_line`，下排候选是新写的。
与候选窗最大的差别是**高度不按内容量**——候选窗的宽高由内容算，候选条宽度是屏幕宽、高度由
`Renderer::bar_height(theme, composing)` 算死。那个 `composing` 是唯一会让高度变的东西：

- **组句当中是定值**，候选从 0 个变 6 个不动（不然每敲一键都顶一下应用）
- **没组句时是 0**：整条收起来，省下的高度还给应用。壳收到空字节串要把视图上那张位图**撤掉**
  （`setBar(null)`），光不更新不够——旧位图还占着高度。视图一矮，安卓自己会把输入法窗口缩回去

高度里还留了一段 `TOOLBAR_HEIGHT`（现在 0）给常驻工具条，将来不受组句与否影响。

**组句状态一变，整块视图的高度就变**，同一根手指在视图里的 y 会平移一个候选条的高度——
真机上安卓每一拍都按当前坐标报，所以 `y - bar_pixels` 这个换算依旧对得上；但**测试里用事先算好的坐标就会打偏**，
多指那几个用例因此要重新取一次点位（`tests.rs` 里两处，注释写了原因）。

下排每格宽度均分（触摸面积一样大）、格间留一条缝，词太长就截断补省略号；
`×` / `‹` / `›` 与每个候选的位置一并返回，供命中测试。

**空格横滑移光标**（2026-09-20）：`Keyboard::touch` 返回 `Option<Fired>`（`Fired::Key` 或
`Fired::MoveCursor(格数)`）——移光标不是「某个键」，翻不成 `action::on_key`。
**拖动当中就走**（`Move` 里越过死区先走一格，之后靠 `cursor_tick` 每拍走），
速度随位移线性涨（死区 8 点 → 0.2 格/拍；80 点以上 → 2 格/拍，一拍是壳的 50ms 心跳）。
`cursor_tick` 用小数累加器摊平——慢的时候几拍才够一格，几拍下来的总位移是对的。
判定排在 `sliding` 之前（空格键宽，划出去不算取消）。
`Session::move_cursor` 把格数摊成一串 `Command::MoveLeft` / `MoveRight`（组句当中直接忽略）。
壳那边**攒成净位移调一次 `InputConnection.setSelection`**，不是每格发一个键——
每格问一遍光标在哪儿要多花几十次跨进程往返。
**不能用 `KEYCODE_DPAD_*`**：那是焦点导航用的，光标到头时会往上冒、把焦点挪到界面按钮上。

**键预览气泡**（2026-09-20）：画在 `renderer/keyboard/popup.rs`（`render_key_popup`），
圆角块 + 阴影 + 放大的字/图标，**单独一张小位图**——它要弹到键盘上方，画在键盘那张里会被窗口裁掉。
安卓侧 `KeyPopup.kt` 用 `PopupWindow` 承载，位置由 `Keyboard::popup_origin` 算好（视图相对像素）。
两个坑：**`PopupWindow` 是输入法窗口的子窗**（`mParentWindow=InputMethod`），坐标相对父窗、
**不是屏幕**（按屏幕坐标传会整块跑到屏幕外）；传的必须是**位图**左上角，不是**内容**的
（位图四周留着阴影留白）。空字节串表示「这个窗现在不该在」，与候选条一个规矩。

**彩色 emoji 与系统字体**（2026-09-18）：swash 只读 COLR **v0** 的基字形 / 图层记录（`swash/src/scale/color.rs` 的 `layers()`），
而安卓 15 起自带的 `NotoColorEmoji.ttf` 是**纯 COLR v1、v0 记录为 0**，三条路于是全落空：`ColorOutline` 读不到图层 →
`ColorBitmap` 没 CBDT/CBLC → 退到矢量轮廓，可 COLR 字形的基字形**本身没有轮廓**（可见部分在图层里），
最后得到一张 0×0 的空图。Windows 的 `Segoe UI Emoji` 同样是 v1 却能画，是因为它额外保留了 3365 条 v0 记录。
**修法是随包带一张位图格式（CBDT/CBLC）的旧版 NotoColorEmoji**，渲染器走彩色位图那条路：
`FontLibrary::system_with_emoji_fonts` 拿壳给的路径**顶替**系统那几张（不能追加——两边字族同名 `Noto Color Emoji`，
都在库里按哪张说不清）。字体在 `assets/emoji/`，来源、许可、升级注意都写在那里的 README。
`examples/emoji_probe.rs` 是查这类问题的工具：`cargo run -p qingjian-render --example emoji_probe -- <字体文件>`
打印每个字形的 content / 尺寸 / 数据量——画得出来是 `Color N×N`，画不出来是 `Mask 0×0`，一眼可比。
`fonts/mod.rs` 的 `bundled_emoji_font_rasterizes_in_color` 把「随包那张必须画得出彩色」钉成回归测试。

**APK 是自包含的**（2026-09-18）：词库与 emoji 那几张都打在 assets 里，输入法首次唤起时自己解到应用私有目录，
**不需要 adb 往 `filesDir` 里推**——这样 APK 单独装到手机上就能用。机制在 `QingjianImeService`：

- emoji 字体与 emoji 表来自仓库的 `assets/emoji/`（`app/build.gradle.kts` 把那个目录整个挂成 assets），
  词库由 `scripts/build.sh` 现打进 `app/src/main/assets/dict.qj`（打进仓库的那份在 `.gitignore` 里）
- 解出来的位置：词库 `filesDir/dict.qj`，emoji 那几个在 `filesDir/emoji/`（`Session::open` 的 `bundle` 参数收这个目录）
- 要不要重解，看标记文件里记的 **APK 安装时间**（`PackageInfo.lastUpdateTime`）——升级一次自动重解一遍，
  不用维护版本号常量；字体 10 MB，不这么记每次启动都要白拷

安卓的字体加载在 `fonts/android.rs`（`#[cfg(target_os = "android")]`）：硬编码 `/system/fonts` 清单 + `read_dir` 兜底，**另带一份 `Fallback`**——
cosmic-text 在安卓上的平台回退表是空的，不自己给的话 `NotoSansCJK-Regular.ttc` 这个字族集合里的中文会落进日文字形，照抄它的 `han_unification` 按脚本选面。

## apps/cli

测试工具，`cargo run -p qingjian-cli -- kaifa`。

- `--predict` 强制开云联想并等结果打印，交互模式下上屏后也联想。
- `--typing` 逐键计时（性能测试用 release 构建跑，目标每键 10 ms 以内）。
- `--chinese-first` 打开中文优先（`[general] chinese_first = true` 的排法），配合 `--replay` 比两种英文词位置。
- `--replay <input-log.jsonl>` 回放评测：把日志里每次上屏的键重新喂给引擎，按来源算首选 / 前五命中率、平均名次、不在候选的条数，打印没命中的例子（`--misses N`）；
  只在内存里学习不写文件，加 `--user-dict` 可带上现有学习数据。
- `--tune 名=值`（逗号分隔）覆盖个人 n-gram 插值与敲错代价的常数扫网格（名字见 `apps/cli/src/tuning.rs`，Core 侧是 `Engine::set_interpolation` / `set_typo_costs`，壳只用缺省值）。
- `--eval-text <文本>...` 整句评测：把用户自己写的中文文本按标点切句、按词库读音转成全拼，冷启动喂给引擎看整句能不能还原原句
  （首选命中率 / 字准确率 / 查询耗时；不依赖日志里当时选了什么，给整句排序与语言模型的改动当尺子），`--eval-save` 冻结成 `句子\t拼音\t上文` 三列文件，
  之后直接 `--eval-text` 它保证比的是同一份句子（本机的在 `data/eval/sentences.tsv`）。排序、整句、纠错的改动先跑它们再合。

## apps/macos

IMK 输入法，源码按 `app / host / imk / candidates / menubar / preferences` 分目录。

- 输入法菜单（状态项 + 系统输入源菜单）与偏好设置窗口都是配置文件的前端：只写 `config.toml`，`Host::apply_config` 一条通路热加载，激活期间每秒看一次文件 mtime。
- `apps/macos/scripts/bundle.sh --install` 打包安装到 `~/Library/Input Methods/`（开发用），`--pkg` 做分发用的 pkg（装 `/Library/Input Methods/`，postinstall 跑 `qingjian-macos --register`
  注册、启用并切成当前输入源；签名 / 公证靠 `QINGJIAN_SIGN_IDENTITY` / `QINGJIAN_INSTALLER_IDENTITY` / `QINGJIAN_NOTARY_PROFILE`，没设就 ad-hoc；`QINGJIAN_TARGET` 指定架构，
  成品 `target/pkg/Qingjian-<版本>-<arm64|x86_64>.pkg`）；`scripts/uninstall.sh` 卸载。
- 日志在 `~/Library/Logs/Qingjian/`（按天分文件留 7 天，删了会重建），用户数据与配置在 `~/Library/Application Support/Qingjian/`。
- 配置项：云联想 `[predict]`（偏好设置「云服务」页有「测试连接」按钮：`qingjian_predict::ConnectionTest` 起线程发一条最小请求，`Host` 用独立定时器 `CloudTestMonitor` 轮询结果显示到窗口底部；
  `reasoning_effort` 缺省 `none`，DeepSeek V4 默认思考，不关正文为空）；模糊音 `[fuzzy]` 默认都关；`[general]` 学习语言（`off` 不显示译文）/ 每页候选数 / 翻页键 / 外观 / 竖排横排 / 拼音显示位置 /
  英文模式候选开关 / 中文优先 `chinese_first` / 双拼方案 `shuangpin`（小鹤 / 自然码 / 微软 / 搜狗，空为全拼）/ 日志级别 `log_level`（缺省 info 不含敲的内容，debug 逐键记，热切换）/ 输入日志 `input_log`；
  `[shortcut]` 模式键 v / u、`question_mark`（缺省关，开了空缓冲区敲 `?` 进问字）、上屏第一 / 第二个译词的修饰键 `translation` / `translation_second`、删候选 `delete_candidate`（缺省 shift，用户词整删、词库词清学习）、翻译选中文字 `translate_selection`；
  `[apps] english_candidates_off` 按 bundle identifier 列出英文模式不给候选的应用（缺省终端 / 编辑器 / IDE，`*` 前缀匹配）；
  `[dictionaries] domains` 打开随包的领域词库（`Resources/dicts/` 11 本，缺省只开 `idioms`），`disabled` 关掉用户目录 `dicts/` 里的某本导入词库；
  偏好设置「词库」页随包的可开关、导入的可开关 / 移除，可导入 TSV / Rime yaml / .qj。
- 系统文本替换（系统设置「键盘 → 文本替换」）：`host/config/text_replacements.rs` 从 `NSUserDefaults` 全局域读 `NSUserDictionaryReplacementItems`
  （每条 `{ on, replace, with }`），激活输入法时重读，变了就经 Core `merge_replacements` 并进配置里的自定义短语再 `set_custom_phrases`；
  `[general] system_text_replacements` 开关（缺省开，「自定义短语」页勾选框），内容可能含证件号、地址，日志只记条数。
- 输入法进程由 launchd 拉起，看不到 shell 的环境变量：密钥写进配置同目录的 `.env`（`QINGJIAN_API_KEY=...`，输入法启动时 dotenvy 读入）或 `config.toml` 的 `api_key`。
- 本地整句模型：`bundle.sh` 把 `data/model/`（或 `QINGJIAN_MODEL_DIR`）三件套打进 `Resources/model/`，用户目录 `model/` 优先；`host/model/mod.rs` 在后台线程加载并预热（首次 Metal 编译）后
  `set_async_sentence_scorer` 接上，`refresh` 每键先读应用光标前 64 字给 Engine 当前文、查询后 `schedule_rescoring`，`RescoreMonitor` 停键 80 ms 请求、20 ms 轮询，
  结果到了重查一次只重画当前页（翻过页 / 动过高亮不动）；「云服务」页有开关（`[model] enabled`）。
- 端到端验证可用 `osascript` 的 System Events 往 TextEdit 发按键再读回文本（终端需要辅助功能权限；输入法得在中文模式）。

## apps/windows

一个产品两个 package：`server`（Server 进程：IPC 分派 + Engine + 命名管道 + 自绘候选窗与悬浮状态条）与 `tsf`（TSF 文本服务 DLL，lib 名固定 `qingjian_tsf`），
外加 `settings`（WinUI 3 设置程序）与 `installer`（Inno Setup）。不合成一个 crate，因为 DLL 不能带 Engine 的依赖树，见 `apps/windows/README.md`；
协议类型在 `qingjian-platform::protocol`，设计见 `docs/design/architecture.md`「Windows：TSF」。

TSF 原有数字 / OEM 标点 / 空格键码按当前布局用 `ToUnicodeEx` 解析（bit 2 避免改变键盘状态），
仅接受单个非代理项 UTF-16 单元。字母、小键盘和 AltGr 处理不变，不保证组合音符输入。

词库导入（设置「词库」页）走 `qingjian-dictionary::import` 转成 `.qj`（空词库拒绝），多选批量、成功的从 `[dictionaries] disabled` 摘掉、页面显示每个文件的结果；
Server 每次轮询比对用户 `dicts\` 的路径 / mtime / 长度快照，配置没变也重载新增、同名更新与移除；配置解析失败时词库沿用上次有效的开关（#36）。

## apps/android

安卓输入法壳（`InputMethodService`），跟别的壳一样只做两件事：把触摸翻译成 Core 的输入、把渲染器出的位图贴到输入法窗口。
JNI 入口是 `Java_app_qingjian_android_QingjianNative_*`，与 Kotlin 侧 `QingjianNative.kt` 一一对应（类名与包名参与符号名），**改一边必须同时改另一边**。
设计与取舍见 `docs/design/keyboard.md`。

- **位图过 JNI**：`surface::encode` 出「8 字节头（宽高，各 u32 大端）+ 预乘 RGBA」，Kotlin 侧 `Bitmap.createBitmap(w, h, ARGB_8888)` + `copyPixelsFromBuffer` 原样吃下——
  `ARGB_8888` 的**内存布局**就是预乘 RGBA（`ARGB` 只是 `getPixel` 那套打包的说法），既不换通道也不重新预乘。这条当初用一次性探针在本机与设备上实测确认过（探针已删，结论留着），别靠记忆。
  **不要用 `setPixels(int[])`**，那条路径假定非预乘。
- **命中测试在 Rust 里**：`Session::touch` 返回位掩码（`flags::BAR` / `KEYBOARD` / `COMMIT` / `PREEDIT`）告诉壳哪些面要重取，位图只在该面脏时重画。
  渲染器只回「按了哪个键」，「按了键干什么」（喂引擎、上屏）留在壳里。
  **触摸坐标不分块**：候选条与键盘共用一个 y 轴（候选条在上），按下时按 y 分派（`Session::touch` 转给键盘、`Session::touch_bar` 管候选条），
  所以显示面必须画在**同一个 View** 上，不能用两个子视图去拼。
  **一根手指归谁，由按下时落在哪半边定**，之后移动与抬起都送回同一家——手指可能已经划到另一半边上了，
  按当前坐标重新分派会让这一下凭空消失。两边各自记自己那批 pointer，不认识的不理，所以 `Session` 不必再记一份归属。
- **键盘单独一层（`src/keyboard.rs`，2026-09-18）**：布局、位图、命中、按下状态机都在 `Keyboard` 里，
  `Session` 只用 `set_metrics` / `surface` / `touch` / `mark_dirty` / `dirty` / `height` 跟它打交道，
  Shift 与中 / 英在画的时候借给它——**这两项引擎也要用**（决定大小写、走哪条路），所以存在会话里，键盘自己只记「哪个键看着是按下的」。
  这样换键盘实现只动这一个文件：将来若改用安卓原生控件拼键盘，`Session::keyboard` 置 `None`、位图那条路自然断掉。
  **多指那套状态机也跟着分成两份**（`keyboard/presses` 与 `Session::pressed`），两边的判定**不一样**：
  键「还落在同一个键上」就一直算按着（键大，抖几像素不该掉字），候选条「挪出触摸阈值」才算没挪窝（横向拖是翻页手势）；
  共用的 `within_slop` 在 `src/touch.rs`，阈值那点事只留一个版本。
- **按键震动（`KeyFeedback.kt`，2026-09-18）**：**直连马达**（`VibrationEffect.createOneShot`，20 ms），
  **不走 `View.performHapticFeedback`**——那条路要经「视图 → 窗口 → 系统」三层转手，任何一层不买账都是
  **静默不震**：真机上带着 `VIRTUAL_KEY` + `FLAG_IGNORE_GLOBAL_SETTING` + `VIBRATE` 权限照样不震，
  而同一台机器 Gboard 震得好好的（说明马达与系统那层没问题）。直连只有一步，成不成一眼看得出来。
  候选条那一按不震（那是点选项，不是敲键）。清单里要 `VIBRATE` 权限。20 ms 是起点、待调；
  「长按与抬起力度不同」这类花样等有设置项再说（也没有读系统那个触摸震动开关：读它是一次跨进程查询，
  每敲一下一次太贵）。
- **按键语义在 `src/action/`**：渲染器报的 `KeyId` / `BarHitId` 先翻成 `Act`（纯翻译，不看状态、能单独测），
  再由 `Session::apply` 按引擎状态执行——退格有拼音就删字母、没拼音就把退格交给应用；空格有候选就上屏、没有就当空格打出去。
- **交给应用的东西分两类**：上屏文本走 `commitText`（`takeCommit`），删字与回车走原样按键 `sendKeyEvent`（`takeCommands`）——
  删字符不能让输入法代劳（它不知道光标前后有什么），回车在有些应用里是提交而不是换行。
  拼音用 `setComposingText` 镜像（`takePreedit`，空串表示结束组字）。**顺序是先上屏再镜像**：
  候选比输入短时（`kaifazhe` 选「开发」）剩下的拼音还在缓冲区里，镜像晚了就丢了。
- **翻页**：候选条右端 `‹ ›` 可点，从候选条起手横向划够 40pt 也翻页（往左是下一页）。划动只认从候选条起手的，键盘上划走不算。
- **换应用时丢掉没上屏的拼音**：`onFinishInput` 里清一次，免得在 A 应用敲的拼音跑到 B 应用里。
- **中 / 英切换是「英文直输」**（2026-09-18）：英文模式下字母不进组句缓冲区，带上 Shift 的大小写直接打给应用，
  标点保持半角。**引擎的英文候选要另外喂一张英文词表**（`Engine::with_english`），安卓这边还没随包带，
  所以给不了候选——这也是别的壳关掉英文候选时走的那条路。要接候选得先把英文词表生成出来随包带上。
- **标点**：组句中打标点先把高亮候选上屏（`nihao` + `，` 得到「你好，」）。桌面是把标点收进「英文直输段」
  （`nihao,` 整串一起算），那是给实体键盘的，触摸键盘上不是这个预期，所以这里不跟。
- **Shift 单击锁定**、再击解锁（桌面才是按住）；中文模式下 Shift 只改键帽，不改喂进去的拼音。
- **输入行为可以在电脑上验**：`session/tests.rs` 是端到端的——喂真实坐标、点位从渲染器真正画出来的命中矩形里取，
  走完整触摸链路，断言上屏了什么、拼音剩什么。**不需要模拟器或真机**，所以「能打字能选词」这条验收是
  `cargo test` 可断言的，改渲染器时它先炸。
- **窗口高度要 Rust 告知**：`configure` 把整块输入视图的高度（候选条 + 键盘 + 底部让开的一段）回传给壳；不说的话输入法窗口会被撑满整屏。
  视图初始高度是 0，而**安卓不给 0 高的视图发尺寸变化回调**，所以配置按屏幕宽度做、不等 `onSizeChanged`，等它就是死锁。
  视图高度按两张位图加起来的像素自己量，不去算点与像素的换算。
  底部被系统手势条占掉的高度从 `WindowInsets.systemGestureInsets` 取，交给渲染器让按键往上让、背景仍铺到底。
- **候选条一页 5 个**（桌面 9 个）：360pt 宽的屏幕里一页 6 个时三字词会被截成「你…」，5 个才留得下格与格之间的缝。
- **日志接到 logcat（2026-09-18）**：`tracing` 只是个门面，没人收就什么都不发——安卓这边**一直没装订阅器**，
  所以 `qingjian-core` / `qingjian-render` 里那些 `info!` / `warn!` / `error!` 全被丢掉，出问题时 logcat 里只剩
  Kotlin 那几句，Rust 这半边是瞎的。现在 `src/logging.rs` 把它接上：
  - 标签与 Kotlin 的 `TAG` 同是 `Qingjian`，`adb logcat -s Qingjian` 两边都收得到；行首带 `INFO qingjian_android::…` 的是 Rust 的
  - 级别映射成 logcat 的优先级（`make_writer_for` 拿得到事件的 `Metadata`），`error!` 能在 `Qingjian:E` 里筛出来
  - `without_time` + 关掉 ansi：logcat 自己带时间戳
  - 一条事件一个 writer，**攒到换行才发**——fmt 那一层一条事件要 `write` 好几次，来一次发一条会被拆成好几行 logcat；
    超过 logcat 的单条上限就分段发，不让它截
  - 级别写死：release 是 `INFO`、调试包是 `DEBUG`。安卓上没地方设 `RUST_LOG`
  - 装的位置是 JNI 的 `open`、**在任何会打日志的调用之前**——建会话失败那几条最需要日志，装晚了正好错过；
    用 `try_init`，重复调用是空操作
  - `android_log-sys` 与 `tracing-subscriber` 都挂在 `[target.'cfg(target_os = "android")'.dependencies]`，
    宿主机跑 `cargo test` 不编它们
- `scripts/build.sh`：cargo ndk 编 .so 到 `jniLibs/` → llvm-strip → 打词库（`dict-convert --out-dir target/android`，不动仓库文件）→ gradle assemble；
  `--install` 顺带装 APK、推词库、`ime enable` + `ime set` 切过来。SDK / NDK / gradle / JDK 都能用环境变量覆盖，不设就自己找。
  词库按 `data/generated/dict.qj` → `data/generated/dict.tsv` → `assets/lexicon/dict.tsv`（9.3 万条）→ `assets/sample/dict.tsv`（148 条）依次退：
  产品数据没生成时用基础词库，翻页这些才验得出来，样例只够验通路。
- **Git Bash 的坑**：`adb push` 与 `adb shell cat` 的 `/data/...`、`/sdcard/...` 会被 MSYS 自动转成 `C:/Program Files/Git/...`，
  设备路径一律加 `MSYS_NO_PATHCONV=1`，本机路径给 adb 前用 `cygpath -w` 转过去。
- **`am force-stop` 会把默认输入法打回系统自带**：换了词库要让输入法进程重开才能重新加载，重开之后得再 `ime set` 一次青简，
  否则后面敲键盘都落在 Gboard 上、`logcat -s Qingjian` 一条日志都没有。
- 模拟器配方：Android 16 / x86_64，**稳定版 37.1.11**（Canary 37.2.9 起不来，带 `metadata` 分区 bug）+ 首次 `-wipe-data`。两个都得有，只做一个照样起不来。
  回归先在模拟器上做（无窗口、可 `adb exec-out screencap` 截图），真机只做最后确认——模拟器是 x86_64、真机是 arm64，两边都要编。

## assets

- `assets/sample/`：手写样例词库与释义表，不是产品数据。
- `assets/emoji/emoji-zh.tsv` / `emoji-en.tsv`：Unicode CLDR 中文 / 英文 annotations 转出的 emoji 表（Unicode License v3，可发布；中文词与英文词各配 emoji，两张表加载时合成一张），
  `cargo run --release -p qingjian-dict-convert -- --out-dir assets/emoji emoji --language zh data/cldr/annotations-zh.json data/cldr/annotationsDerived-zh.json`（en 同理）。
- 英文词表词频：`uv run tools/corpus/english_frequency.py data/generated/english.tsv -o data/generated/english-frequency.tsv`，再 `... english <词表> --frequency <那个文件>`。

## tools/gloss-gen

用 LLM 批量生成释义表：`cargo run --release -p qingjian-gloss-gen -- generate`（密钥读 `QINGJIAN_API_KEY`，结果 JSONL 在 `data/generated/`，不进 git、可续跑，`--limit 80` 试跑）
再 `... export`（写 `glossary-{en,ja}.tsv`，产品数据在 `assets/glossary/`，见那里的 README；格式 `词\t词性. 译词[|假名]`）。CLI 与 bundle.sh 用的就是这两个文件。

## tools/dict-convert

产品数据的生成工具，输出到 `data/generated/`（gitignore）。

- `lexicon`：从 `assets/lexicon/`（自建词库源：规范字 + 常用词 + THUOCL 领域词）加 Unihan 读音（`data/unihan/Unihan_Readings.txt`）、LLM 多音字标注（`gloss-gen pinyin`，
  结果 `data/generated/pinyin-llm.jsonl`，不进 git）、语料词频（`lm-unigram.tsv`）建基础词库 `dict.tsv`（8.7 万条），并把 THUOCL 领域词按语料次数 < 50 拆成
  `dicts/<领域>.tsv` + `.qj`（11 本、13 万条，`--domain-keep-min`），流程见 `assets/lexicon/QINGJIAN.md`；`--extra-words` 并入人工挑的领域词 `assets/lexicon/domain_words.tsv`。
- `english`：转 `assets/lexicon/05_english/00_all_words.tsv`；`cedict`：释义表备用来源。
- `bigram`：统计语料；`--phrases` 给短语层、`--brand` 给品牌词（`assets/lexicon/brand.tsv`，青简 210）与中英混杂词（`mixed_words.tsv`，C盘 / B站：合成计数要成分词在语料里，C 不是 token，只能直接给一元，次数对着同音竞争词定），领域词也走合成计数（语料里只有几十次的词当 token 统计会吸走成分词的二元证据）。
- `mine`：从语料挖词库没收的高频词并过滤（`oov_filter.rs`：虚词规则 + 相邻字对 PMI≥3，`--candidates` 只重过滤）。
- `phrases`：挖短语层（两遍扫语料：相邻两词、两段二元都够频的相邻三词，总次数与对话语料次数都 ≥ 2000 + 边界规则，读音由成分词拼出；我的 / 不知道 / 有没有 这类常用词表不收的组合，
  `assets/lexicon/phrases.tsv`；词库已并入过短语时重跑加 `--refresh`）。
- `pack dict|lm|glossary`：打 `.qj`（释义表也进容器）。
