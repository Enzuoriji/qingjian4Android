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

`Config`（TOML 配置文件，`[general]` / `[shortcut]` / `[fuzzy]` / `[dictionaries]` / `[apps]` / `[predict]` / `[keyboard]` 分节，首次运行写模板，
`set_value` 用 toml_edit 原地改键保留注释；`[model] enabled` 本地整句模型开关，`LocalModelConfig`；
`[keyboard]` 是按键震动（`KeyboardConfig` + `VibrationStyle`，**只有安卓用**，2026-09-22）；
**模板与缺省分平台的有三处**：`[shortcut]` 的修饰键、`[apps]`、`[dictionaries] domains`
——安卓的领域词库**缺省 11 本全开**（候选条是横滑的，候选多不挤），桌面仍只开 `idioms`，
两条分支在 `DictionariesConfig::default` 与 `template_domains!` 里，测试盯着它们与各自平台一致）；
`extra_dictionaries` 列出 / 加载随包领域词库与用户 `dicts/`
（mac 壳与 Windows Server 共用，同名 `.qj` 优先于 `.tsv`）；`protocol` 模块是 Windows Server ↔ TSF DLL 的 IPC 协议类型
（`ClientMessage` / `ServerMessage` / `Frame` / `PreeditSegment`，全 serde，两端共用，见 `docs/design/architecture.md`「Windows：TSF」）。

## crates/qingjian-render

自绘渲染器：候选窗一帧 + 主题 → 预乘 RGBA 位图，tiny-skia 栅格 + cosmic-text 文字（fontdb 按平台清单只加载几个字体文件、不扫系统），
自己解析 `trak` 字距表、按主题 gamma 加深笔画；cosmic-text 打了 `opsz` 光学字号补丁（qingjian-team/cosmic-text 分支 `qingjian-opsz`，workspace `[patch.crates-io]` 钉 rev）。
`examples/preview.rs` 出 PNG 与真机截图并排比、`--measure` 与 AppKit 对宽度。
候选条（安卓专用，`renderer/bar/`）：**最下面那行左边译文、右边云联想给的整句补全**
（`draw_bottom_line` / `draw_bottom_sentence`，2026-09-22）。高度按
`bar_height(theme, composing, bottom_line)` 算——第三个参数是**会话级**的「那行要不要留」
（挂了释义表 **或** 云联想开着），不是「这一屏有没有东西可画」；
拼音行仍复用候选窗那套 `draw_top_line`，只是候选条传 `with_trailing = false` 把右截让给下面那行。mac 壳 `candidates/bitmap/` 贴位图，`[general] renderer = "system"` 切回 AppKit 绘制
（过渡期退路，偏好设置「候选窗口」页可选）；`[general] font` 是候选窗字族名（空为系统字体，`bitmap/font_files.rs` 用 CoreText 按字族名找文件只加载那几个，没装就回系统字体；
设置页 `preferences/font_picker/` 是搜索框 + 列表）。设计与验收见 `docs/design/rendering.md`。

软键盘走同一条路：布局数据在 `keyboard/`（`KeyboardLayout` 按「单位宽」算几何，每行居中，第 2 行自然得到半键错位，不写死坐标）、绘制在 `renderer/keyboard/`、
主题在 `theme/keyboard.rs`；`render_keyboard` 与 `render_status` 一样，出位图的同时把**每个键的命中矩形**一并返回。设计与取舍见 `docs/design/keyboard.md`。

**键盘高度按屏幕算**（2026-09-20，K9）：`KeyboardTheme::fitted(screen_height, landscape)`
把高度定成屏幕当前方向那一维的 **30%**（横屏 **49%**），夹在 `[200, 300]` / `[170, 260]` 之间。
改之前是写死的 202 / 176，大屏手机上偏矮——而那两个数本来就是 fcitx5 那套比例
（竖屏 30%、横屏 49%）按一台小屏手机换算出来的，所以定值法必然在大屏上偏矮。
**上限是给平板留的**（当初不敢用百分比就是怕平板横屏八百点高算出巨无霸），
下限贴着那两个老定值，小屏不至于更矮。
`screen_height` 由壳算好报进来（`configure` 的第二个参数）：**竖屏取长边、横屏取短边**，
不直接用 `displayMetrics` 的高度——有的 ROM 转屏后它还是报竖屏那个值
（`QingjianImeService.screenHeightPoints`）。横竖屏也由壳判断后一起报。

**候选条上那行译文**（2026-09-22，E4）：组句时的高度从 66 点变 **81 点**，多出来的 15 点
就是候选底下那行小字——`annotation_band_height` = `annotation_font.line_height`（15），
**不带 `row_padding`**：那一行上下已经有候选行的下留白与条子自己的下留白，再加就顶出条子了。

- 只画**高亮那个**候选的译文（`docs/design/candidate-ui.md` 定的「横排只给高亮的那个在下面
  单独一行显示」），起点对齐高亮那格的左边缘；一段段顺着画，画到右边缘就截断补省略号
  （条子通栏，注解爱多长有多长）。
- 颜色按 `Tone` 走主题里现成的三档：译词 `gloss`、词性 `pos`（比译文更浅）、生词 `fresh`。
- **高度是会话级的**：`bar_height` / `render_bar` 都多收一个 `annotations` 布尔，
  **不能**按「这一屏有没有译文」临时定——条子一变高上面的应用就被顶，滚一格跳一下不能接受
  （`docs/design/keyboard.md` 的「敲一个键不会顶动应用内容」）。那个布尔在会话建立时定死
  （挂了释义表就为真），两处共用同一个 `composing_height`，天然一致。
- **只有安卓壳与预览脚本吃这个改动**：`render_bar` / `bar_height` 的调用方只有
  `apps/android/src/session/mod.rs` 与 `examples/preview.rs`——macOS / Windows 各有各的画法。
- **那行小字可点**（2026-09-22）：点它上屏**译文本身**而不是候选词。
  `BarHitId::Translation(sense)` 带着**第几条**，**一行两条译文时点哪条上屏哪条**——
  义项边界认的是那条 ` · ` 分隔符（三个壳拼 annotation 时都这么隔），
  分隔符与词性那些 Faint 片段不归任何一条、点它们不响应。
  每条的范围**按它真画出来的那截文本**给（左右各放 `ANNOTATION_HIT_PAD` 4 点；
  从前的 8 点会让相邻两条叠上），不是整条宽度——不然右半边那一大片空白也成了靶子。
  动作链是照旧那三跳：渲染器回 `BarHitId` → `action::on_bar` 翻成 `Act::CommitTranslation(sense)` →
  `Session` 调 `Engine::commit_translation(&candidate, sense)`（学习记账与拼音消耗引擎按
  「选了那个候选」办，壳只管把返回的文本交出去）。
  **`Candidate.reading` 对中文候选恒为 `None`**（核心里写着「中文候选暂不使用」），
  所以那行的次序就是义项次序；哪天真给它填上读音了，切靶子这条规则要跟着看一眼。

**键盘上那几个图标（2026-09-21 换成现成的）**：⇧ 大小写、⌫ 退格、工具页那格的剪贴板，
路径来自 `assets/icon/material/` 里那几张 Google **Material Symbols**（Apache-2.0）的 svg，
由 `python assets/icon/render-key-icon-path.py` 转成 Rust 代码（生成物
`renderer/keyboard/icon/path.rs` 随仓库提交，画法在 `renderer/keyboard/icon.rs`）。

- **别手写坐标**：在这之前这几个图标是我自己 `move_to` / `line_to` 拼的，用户看了说
  「不要这样做去网上找可以用的」。选 Material 是因为它在安卓上随处可见、许可干净。
- 路径是 **960×960、y 轴朝上为负**那套坐标系，原样搬过来；渲染时按**每个图标自己的包围盒**
  缩到「最长边 = 目标边长」（`path::BOXES`）——960 网格四周是 Google 留的呼吸位，
  照网格缩会比要的尺寸小一圈。
- 图标靠**子路径方向**挖空（外轮廓一个方向、内轮廓相反），非零填充规则自动出镂空，
  不必再自己用 `BlendMode::Clear` 打孔。
- 解析 svg 的是 `assets/icon/svgpath.py`（`M L H V C Q T Z` + 相对命令 + 隐式重复 +
  `Z` 之后当前点回到子路径起点），与 `render-logo-path.py` 共用一份。

**候选条那个标（2026-09-21）**：没组句时那条细的最左边，画的是青简的标——`src/logo/`。
路径数据是生成的（`assets/icon/render-logo-path.py` ← `assets/icon/menu.svg`，见那个 README），
画法与 `gear.rs` 一样：**画路径不画字形**，先画在一张独立小图上再整张叠到画布。

- **只画四片竹简**（`menu.svg` 里那枚键帽的第一段**不画**）：2026-09-21 用户说「灰色的边框
  去掉」，那条上就只剩竹简。颜色是 App 图标那套绿（亮 `#94BE52`、深 `#336F33`，左上那片是深的）。
- **按竹简自己的包围盒缩，不按 viewBox**：`menu.svg` 的 39×28 里竹简只占中间一小块
  （约 12×22），照 viewBox 缩的话四周全是留白、标看着就小。包围盒由生成脚本算好写在
  `path::SLIPS_BOX` 里（照曲线真正走到的点算，不是拿控制点——控制点会外扩）。
- **用四片的那个形、不用 App 图标的六片**：App 图标（`logo.png` / Windows 的 `qingjian.ico`）
  是 2 列 × 3 行**六片**细竹简、内容框 1:3.4 的竖条——缩到这条 36 点高的条子里每片只剩
  4×7 点，糊成一团。
- **那条细的 36 点高**（`IDLE_HEIGHT`，2026-09-21 从 30 抬上来的，用户要「条子宽点、
  给标和下面 q 键留些距离」）：标仍 24 点，上下留白从 3 点变 6 点，不再贴着自己的边。

**剪贴板页（2026-09-21，K10 的一半）**：面板做成**键盘的第四、五页**（`Panel::Tools` / `Panel::Clipboard`）
而不是另起一套绘制——每行 5 个单位，就自动拿到了位图渲染、命中矩形、按下态与切页机制。

- **一行一条**（`CLIPBOARD_CELLS` = 5 条记录 + 一行控制，**六行**，所以这页的行高比别的页矮一截）。
  2026-09-21 用户要「仿搜狗的样式」，从「三行两格」改的：两列时一条只摊到半屏宽，十来个字
  就被截，而剪贴板里多的是长句和网址。
- **一条是一张卡片**：格子本身铺满整宽，画键帽时左右各缩 `CARD_INSET`（4 点）——
  一条通到屏幕两边看着像一整块面板，缩出两条缝才是一条条分开的。命中区还是整行宽。
- **面板的格子是「键盘的键」，字是活的**：`KeyId::Clipboard(usize)` 只是**窗口里第几格**，
  文本从 `KeyboardState.clipboard` 取——那是**会话切好的这一屏几条**（`Session::clipboard_range`
  按滚动量切片），格号直接就是下标。存几条、滚到哪儿、怎么去重都不归渲染器管。
  这一屏没那么多条时 `label()` 给空串，`draw_key` 见空**连卡片都不画**（否则空着一块白卡片）。
- **记录区是一段能滚的窗口，滚的是「格子里的内容」**（2026-09-21 用户要「向下滑动选择」，
  替掉了原先的 `‹ ›` 翻页）：整格那部分由会话换（切好切片），不足一格的那部分由渲染器
  按 `KeyboardState.clipboard_offset` 把整排卡片往上让——**渲染器不做除法**，
  「第几条起」是会话按键盘几何（`Keyboard::clipboard_pitch` ← `KeyboardLayout::row_height`）
  算的，两边各算一次会因浮点差出一格。
  - 会话那边取整要**加一点补偿**（`GRID_EPSILON` = 一格的万分之一）：滚动量是一格格累加、
    又夹在「多出来的格数 × 一格高」上的，浮点下 `3 × pitch ÷ pitch` 可能是 2.99999…，
    floor 之后就少一格，滚到底时最后一格永远差一丁点露不全。
  - 卡片滚出窗口靠**画在一张窗口大的图上再整张贴回去**裁掉（画布本身不裁）——
    与 `logo::draw_logo` 同一个路数。命中区跟着裁，滚出去的那几格点不到。
- **那种格子要左边对齐 + 截断**（`fit()`，与候选条同一个），不能照键帽那样居中——
  一段话居中会两头都被切。文字从**卡片**左边起算（`CARD_INSET` + `CELL_TEXT_PADDING`）。
- **一条都没有时中间写一句**（`BLANK_CLIPBOARD`，`KeyboardLayout::is_clipboard()` 认出这一页）：
  不写的话整块键盘只剩底下那两个控制键，看着像坏了。
- **单位宽取最挤的那一行**：记录行（1 格 × 5 单位）与控制行（2 个 1 单位）一样宽，
  所以卡片铺满整宽、控制行窄一点居中——`layout.rs` 里那两个常量配出来的就是这个效果。
- **标与候选条共用同一块地方**：`bar_height` 两态（组句 = 拼音行 + 候选行；没组句 = 36 点一条），
  `render_bar` 没组句时只画标（`crate::logo::draw_logo`）+ 页码，组句时**不画标**。
  所以「组句当中够不着剪贴板」是结构性的，不是忘了做。
- **剪贴板没有页码**（2026-09-21 起）：`Frame::footer` 只报候选翻到第几屏了。
  跟手滚动里「第几屏」不是一个说得清的东西。
- **数据是落盘的**（`qingjian_learning::Clipboard`，`Session::clipboard`）：壳每复制一条报一次
  （`clipboardChanged`），**去重 / 插最前 / 截到 50 条这些规矩都在那个类型里**，会话只管转发。
  文件是数据目录里的 `clipboard.tsv`（安卓给的是 `filesDir`，`Session::open` 的第四个参数）。
  - **一改就写盘**，不像别的表那样攒着等 `flush`：那些是统计与偏好，丢一两条无所谓；
    剪贴板丢的是「刚复制的那一条」，而输入法进程在安卓上随时会被杀。
  - **一条一行 + 转义**（`\` `
` `
` `	`）：内容是任意文本，不转义的话一条带换行的
    就能把「一行一条」这个格式撑破。这个 crate 里别的表按 `	` 切就行，那些字段是词和数字。
- **敏感与空白在壳那边就滤掉**（`QingjianImeService.readClipboard` 看 `EXTRA_IS_SENSITIVE`）——
  读得到什么、该不该读是平台的事；记几条、怎么记是会话的事。
- **监听器只管「变化」，补一次靠 `onStartInputView`**：输入法起来之前复制的东西收不到，
  每次弹出键盘时补读一次当前剪贴板。**不能在 `onCreate` 补**——那会儿窗口还没显示，
  非前台读剪贴板会让系统弹「某某读取了剪贴板」的提示（Android 12 起）。
- **剪贴板那几格不弹气泡**（2026-09-21 用户要的）：气泡正好压在下面那几条上，按住一条想
  看别的就碍事；那儿也没什么要预览的——一条的内容本来就写在卡片上。
- **删一条是「往左滑、松手」**（`Press.deleting`，`DELETE_SWIPE` = 16 点，比 ⌫ 上滑那个 22 点小）：
  格子本来就靠左边按下去，往左一划就到头了。与 ⌫ 上滑清空同一个手感，气泡也改口说「松手删除」。
- **甩一下会接着滑**（`Session::clipboard_fling`）：与候选条那条带子同一套 [`Fling`]
  （衰减曲线、起手门槛都一样），只是方向竖着——候选条收的是 `velocityX`、
  剪贴板收的是 `velocityY`，`start_fling` 里按「刚才是谁在滚」二选一。
  两个方向的分量是两回事，符号也相反（往上甩要让 `clipboard_scroll` 变大）。
- **「这一拍没动」有两种，别混**：一种是滚到头了（该停），一种是 `dt = 0`（不该停）。
  判断写成 `step != 0.0 && 位置没变`——只看「位置没变」的话，壳偶尔敲一个 0 毫秒的帧
  就会把滑行掐断。
- **滚动与左滑按方向分**（`Press.scrolling` / `SCROLL_SLOP` = 8 点）：**纵向占优的算滚**
  （跟手，每拍都要报 `Fired::ClipboardScroll`），横向往左的算删。认了滚之后这一下就一直是滚。
  滚动时**不弹气泡**（`pressed_scrolling`）——气泡正挡着要看的那份列表。
  阈值比 `DELETE_SWIPE` 还小：滚动要跟手，等滑够十几点才动会很黏。

**表情面板（2026-09-21，K10 的另一半）**：emoji 与颜文字**共用一页**——布局、命中、
滚动、上屏全一样，只是喂进去的数据不同（`Panel::Emoji` 与 `Panel::Kaomoji` 在
`KeyboardLayout::of` 里指向同一份布局 `emoji()`）。工具页加两格进它们。

- **数据两张表**：`assets/emoji/emoji-panel.tsv`（1923 → 滤完 1549 个）与
  `assets/emoji/kaomoji-panel.tsv`（872 条、22 类）。两张都是脚本从**官方数据集**生成的
  （`render-panel.py`，来源与许可见各目录 README），表随仓库提交、原料在 `data/`（不进仓库）。
- **颜文字那张表放在 `assets/emoji/` 下**，虽然脚本在 `assets/kaomoji/`：gradle 是把资源
  目录**平铺**进 APK 的 assets 根（`srcDir` 不保留目录名），两个目录都挂会撞名
  （`mergeReleaseAssets` 直接报 Duplicate resources）。表名带前缀区分就够。
- **格子的字号按内容长短自己挑**：一两个字符的（emoji）用键帽那个大号，更长的
  （颜文字）缩一号才塞得下。
- **加载时按「渲染器画不画得出来」滤一遍**（`Renderer::covers`）：渲染器**只加载清单里
  那几个字体、不扫系统**，颜文字里的 `⑅`、`╹`、`∀` 压根没有字形，摆上去就是一排豆腐块。
  `covers` 用 ttf-parser 逐个字符查 fontdb 的 cmap（fontdb 底层就是它，所以只为这一处
  加了那个依赖），空白与变体选择符 / 零宽连接符放行。
- **颜文字里「本身就是 emoji」的字符也滤掉**（`☺︎`）：字体里当然有它，过得了字形关，
  渲染器就按**彩色 emoji** 画出来，摆在颜文字堆里格格不入。判断用 Unicode
  `emoji-test.txt` 的码点集合，但**只滤一两个字符的那种**——长的几条里夹着 emoji 表里的
  符号是常事（`♥`），不能一竿子打翻。
- **格子能上下滚**：与剪贴板那份列表**同一套**（整行由会话切、零头由渲染器让开），
  区别只在「能滚的那一段」是哪几行——剪贴板是上头五行，表情页是**标签条下面**那三行。
- **踩过的坑（两次都是白费功夫）**：
  - 文件头的说明注释也是 `#` 开头，一律当分类的话面板上会冒出「由 render-panel.py
    生成…」这种标签。**分组一律写 `# group: 分类`**，只认这一种。
  - 画进「可滚的那一段」时要用**那一层自己的坐标**（减掉它的顶边）。剪贴板那一段从
    键盘顶边起，减不减都一样；表情页那一段在标签条下面，不减就整片往下偏一行。

**模拟器上验不了「滑动手势」**（2026-09-21 撞了一轮）：`adb shell input motionevent`
每次调用是**独立注入**的（没有同一个 downTime，Move 会被系统丢掉），
`input swipe` 在浏览器里又容易被页面滚动 / 手势导航吃掉。所以滚动这类**手势**相关的
改动，判断依据只有自动化测试 + **真机上用手指试**——别拿 adb 造的手势当结论，
更别据此去改代码。（上一版追着这个查了半天，最后发现代码是好的。）

**工具页（2026-09-21）**：原先是一整行一个文字键，用户看了说「入口要和搜狗一致」——
现在是**一排排的图标格子**（`TOOL_SLOTS` = 15 格，一行 5 个）＋一整行宽的「返回」。

- **一格 = 图标在上、名字在下**：`KeyId::Tool(index)`，名字从 `TOOLS` 取（现在只有剪贴板），
  图标在 `tool_icon(index)` 里对（`icon::draw_clipboard`）。没排工具的格子名字是空的，
  `draw_key` 见空**整个不画**（与剪贴板那几格同一个判据）。
- **图标收的是边长、不是键高**：键帽上那两个（⇧ / ⌫）按「键高 × 比例」算，
  工具格子是两行（图标 + 名字），多大由 `TOOL_ICON_RATIO`（0.42）×格子高算好再传进去。
- **「返回」用 `Key::fill()` 撑满整行**：这一行只有它一个键，按单位宽算的话两头会各缩
  进去一截（单位宽是照上面那几行 5 个格子的排法定死的）。别的页那个「返回」是五格之一，
  不这样。

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

下排候选是**一条能横着滚的带子**（2026-09-21 定）：每格宽度**按内容定**，只有低于底线
（`MIN_CELL_WIDTH` 48 点，不然单字候选那格细得点不着）才抬上来。**不铺满一行**——带子要能滚，
铺满就没有滚的余地了；这样一来多长的词都装得下。格间留一条缝；
`render_bar` 多收一个 `scroll`（这条带子被拖出去多少像素），每格从 `padding − scroll` 起铺，
画到位图外面自然被裁掉，命中矩形照铺。`×` / `‹` / `›` 与每个候选的位置一并返回，供命中测试。
`bar_cell_widths` 的输出有个硬保证：各格加缝加左右边距**正好铺满一行**，`tests` 里钉着。

**候选格里不画序号**（2026-09-20 去掉）：候选条是手指点的，选哪一格靠位置不靠数字——
序号是实体键盘那套（`⇧ + 数字`）的东西，手指够不着，白占宽度。序号仍在 `Row::index` 里
（桌面候选窗要用），只是 `draw_bar_row` 不画。**腾出来的宽度是实打实的**，
格宽与能装几个字都在 [`docs/design/keyboard.md`](../design/keyboard.md) 那笔账里。

**但去掉序号救不了截断**，算一下就清楚（360 点宽的屏、当时一页固定 5 个）：
`slot = (360 − 2×8) / 5 = 68.8`，减去 `column_gap` 8 得到**格宽 60.8 点**；
不画序号前还要再扣掉序号（11 点字号约 6 点宽）与 `INDEX_GAP` 3 点 → **只剩 51.7 点**，
16 点字号下就是 **3.2 个汉字**——这正是「超过三个字就变省略号」那条线的来历。
去掉序号后是 **3.8 个字**：四个汉字要 64 点，**仍然差一点**。
所以这条只是把线往后挪了半个字，真要装下四五个字的词得动布局（见下条）。

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

**⌫ 上滑清空**（2026-09-20，K7）：⌫ 上**往上滑**（滑上去只是预备）、**松手**把光标前面整段清掉。
`Keyboard::touch` 多报一种 `Fired::ClearToStart` → `Command::ClearToStart`，
壳用一条 `deleteSurroundingText(光标前面有几个字, 0)` 兑现。组句当中不理。
**滑上去之后气泡改口**：`Popup::Text("松手清空")`——气泡本来就能画一句提示
（`Popup::Key` / `Popup::Text`），提示按文字宽度撑开、用键帽那个字号。
**`Keyboard::held` 排掉了正在做手势的手指**——不然长按连发会和滑动手势抢同一个键。
（第一版做的是「往左滑选字、松手删选中的」，用户说不好用，整块换掉了。）


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

**展开选词（K13 ①，2026-09-23）**：候选条**长按**（`Session::repeat` 里查 `pressed` 里有没有这根手指）
弹一块多行网格盖住键盘区，一屏铺三十来个候选、能上下滚。四个要点：

- **面板占的就是键盘那张位图**：`keyboard_surface()` 在 `expanded` 时分流到 `expanded_surface()`，
  所以**壳一行没改**——还是「上面一条候选条、下面一张位图」，触摸也照样按 y 分派
  （`Session::touch` 里 `expanded && y >= bar_pixels` 那一路走 `touch_expanded`）。
- **量宽、画法、行高都与候选条共用**：渲染器 `renderer/panel/` 的 `CandidateGrid` 折行用的是
  `bar_cell_widths` 量出来的那份宽度，每格调 `draw_bar_row` 画——同一个词在两处一样宽。
- **高度照键盘位图算**（`keyboard_height() + bottom_inset`，不是 `keyboard_height()`）：键盘的键
  只排到 `theme.height`，底下那一段是留给系统导航栏的，候选铺满整张图会伸进去被压住。
  候选画在一张**只有可用高度**的小图上再整张贴回来（画布自己不裁，与键盘剪贴板页同一个路数）。
- **滚动上限从这一帧的网格上问**，所以滚动时**不能**把 `expanded_view` 置空当脏标记——
  那样下一拍 `max_scroll` 返回 0、滚不动。用单独的 `expanded_dirty`，位图留着。

**表情面板整页翻（K13 ③，2026-09-23）**：照 fcitx5-android 重做，**推翻了同一天早先那版
K13 ②**（那版滑的是上面那条标签条）。六个要点：

- **分类 → 页**：`EmojiPanel` 多一层分页视图——`pages`（每个分类的条目按 `EMOJI_SLOTS` 个
  切好、首尾相接）+ `ranges`（第 i 个分类占哪几页）。**当前页不单独存**，由位移算
  （`emoji_page()` = `scroll ÷ 页宽` 取整）：跟手拖到一半时「算哪一页」本来就是中间态，
  存两份迟早不同步。内容变过（「最近」多了一条、滤过画不出来的字形）要 `rebuild()` 重排，
  重排之后 `clamp_emoji_page()` 把位移夹回范围内——不夹会指到不存在的页上，屏幕一片空。
- **位移在会话、整页在渲染器**：`Session::emoji_page_scroll` 是**相对第一页的总位移**（像素），
  `emoji_view()` 把 `[前一页, 当前页, 后一页]` 三片和零头一起打包给渲染器；渲染器按
  「(第几页 − 1) × 整宽 − 零头」摆，摆到视口外的自然看不见。整页那部分会话已经换掉了，
  渲染器只挪零头——与剪贴板那份列表同一个分工。
  它写成**自由函数而不是 `Session` 的方法**：借的是面板里的字符切片，而调用处紧接着要
  可变借键盘，走 `&self` 的方法就把整台会话借住了，借字段才拆得开。
- **松手的三档判定照 `PagerSnapHelper`**（`settle_emoji_page`）：手速 `|v| ≥ MIN_PAGE_FLING × density`
  （0.05 点/毫秒 = 50 dp/s，正是安卓 `scaledMinimumFlingVelocity` 那条线）→ 朝甩的方向翻一页，
  **只看方向、不看大小**，也跟拖了多远无关；手速不够就看拖了多远——「离得最近的那一页」是
  `page + (offset > 半页)`，过半页翻、不到半页回原位。目标定了起一段 [`Slide`]。
- **`Slide` 与 `Fling` 是两回事**（`session/slide.rs`）：`Fling` 是「以起手速度指数衰减」，
  停在哪由曲线决定；`Slide` 是「知道要去哪、固定 200ms 滑过去」（ease-out cubic）。
  接口也不同——`Fling::step` 给**增量**，`Slide::step` 给**绝对位置**（半路改目标才方便）。
  拿惯性去凑整页会差一截（速度与目标位置对不上），所以单开一个类型。
  `mask()` 的 `FLING` 位与 `fling_step` 都要带上它，否则动画没人敲帧、停在半路。
- **标签全平铺、只点不滑**：格数跟着分类数走（渲染器对表情页**不走**通用循环，另开
  `draw_emoji_page`），画法与命中都按 `emoji_groups.len()` 均分，**画在哪就点得着哪**。
  一格画图标还是画名字由会话定的 `GroupLabel` 说了算——`GroupIcon` 枚举在渲染器这边，
  **它不认识分类名**（那是随包数据的事）。`GroupLabel::Text` 存 `String` 而不是 `&str`：
  这张表跟着面板长期放着，借分类名就成了自引用，存不下来。
- **标签行下沿一条 2 点的分页细条**（照它的 `PickerPaginationUi`）：宽 = 整宽 ÷ 类内页数，
  位置跟着「类内第几页 + 跟手零头」连续移动；类内只有一页不画。**不额外占高度**（贴在标签行底边里）。

**颜文字页去掉分类条（2026-09-23）**：`KeyboardLayout::kaomoji()` 与 `emoji()` 分家——
前者四行格子、**第 0 行也是格子**（`is_kaomoji()` 就靠「第 0 行有没有 `EmojiGroup`」判），
`of(Panel::Kaomoji)` 不再指向 `emoji()`。配套三处：

- `EmojiPanel` 多了 `slots`（每页几个：表情 15、颜文字 20）与 `sticky_recent`
  （「最近」空了留不留那一类：表情不留、颜文字留——右上角按钮按的**下标**得一直是 0）。
  **`Default` 手写了**：`slots` 默认 0 的话 `rebuild` 会切成「一页一个」，几十条就是几十页。
- `EmojiPanel::merge_groups()` 把 22 个分类并成一条长表（颜文字用）；名字留个占位的「全部」，
  面板上不画它，只用来占住「第 1 类」这个位置。
- 渲染器 `draw_emoji_page` 按 `is_kaomoji()` 分两条路：表情那条照旧画标签行 + 细条，
  颜文字那条格子从**键盘顶边**起，**第 0 格固定是「最近」**（`draw_recent_cell`），
  颜文字从第 1 格起排（`draw_emoji_grid` 的 `start_slot` 参数）。

**按键判定改用「格子的边界」（2026-09-23，修「打字偶尔漏字母」）**：命中区从前只盖住键帽
（`renderer/keyboard/mod.rs` 里 `keys.push(KeyHit { x, width: key_width, .. })`），键帽之间
那条缝**不归任何键**——键盘上约三成的面积是死区，手指落偏一点就一个字都不出（震动照发，
所以感觉是「按到了却没反应」）。现在四个边各外扩半条缝，**只改 `KeyHit`、不动绘制**，
看着一模一样。配套三处：`Press::sliding` 改成**每拍重算**（从前一次置上就永不回头，
滑回来也不算）；`QingjianSurfaceView` 的 `ACTION_MOVE` **逐根手指上报**
（`actionIndex` 在 MOVE 时恒为 0，从前只报第 0 根，第二根手指的移动全丢）；
`MotionAction::from_motion` 只把 3 当 `Cancel`、其余是新的 `Ignore`
（从前 `_` 一律当取消，一个没见过的 action 飘过来就把按着的键全清掉）。

**滑起来之后不画按下态**：`Keyboard::refresh_pressed` 里排掉 `scrolling_x` 的那根手指
（不然键帽上那个放大气泡会跟着手指跑一整路）。注意这一步**必须放在横滑分支 return 之前**——
早先写在分支末尾，而横滑那一路 `return Some(Fired::GroupScroll(..))` 提前走掉了，压根没走到。

**返回键**（加在 K13）：`Session::dismiss` 是唯一一个「这一下归不归我管」的 JNI——
**返回 0 = 不归输入法**，壳照常把返回交给应用去收键盘；收面板、把页切回字母页都一定带着
`KEYBOARD` 那一位，所以「非 0 = 我处理了」成立。壳挂在 `onKeyDown`（不能用 `onKeyUp`：
那会儿窗口多半已经被系统收掉了）。

**用户学习（2026-09-22 接上）**：`Session::open` 的 `data_dir`（安卓传的是 `filesDir`）有值时，
在 `filesDir/learning/` 下开一个 `FrequencyLearner`（主文件 `user.tsv`，几张兄弟表由它推导同目录）
并 `with_learner` 挂上；**任何一步失败都只记日志、退回不挂**——学不了顶多是排得不够顺，输入法起不来是另一回事。
那个目录得自己 `create_dir_all`：`write_atomic` 只写文件、**不建父目录**，省了会一路静默失败
（落盘只在 `FrequencyLearner` 里打一条 warn，从外面看不出没存上）。接上之前安卓**一个字都不学**，
K8 的「长按候选 = 删词」当年删不动正是因为这个（那个功能已整个摘掉，不复活）。

**落盘时机**（`flushLearning` → `Session::flush_learning` → `Engine::flush_learning`，壳侧见 `QingjianImeService`）：

| 时机 | 为什么 |
|---|---|
| `onWindowHidden` | **主路径**。窗口真藏起来时必到 |
| `onFinishInput` | 焦点离开输入框但窗口还在（切换应用、点到别处） |
| `onDestroy` | 进程退出前，**必须在 `close` 之前**（`close` 就是 `drop`，会话没有 `Drop`、不会自己落盘） |
| 60 秒心跳 | 键盘一直开着时的兜底。与电脑版同一个规矩（`LEARNING_FLUSH_INTERVAL`） |

没有脏数据时是空操作（各表按 dirty 位判断），所以多叫几次不要紧。

**⚠️ `onFinishInput` 管不了「收起键盘」（2026-09-22 模拟器实测）**：按 BACK 收起键盘时它**压根不触发**，
`ImeTracker` 只报 `HIDE_SOFT_INPUT_BY_BACK_KEY`，数据最后是等 60 秒心跳兜下来的——最坏要多等一分钟，
而输入法进程随时可能被杀。补了 `onWindowHidden` 之后才是「收起来就写」（实测同秒落盘）。
**别再只挂 `onFinishInput`。**

**为什么不每次选词就落盘**：一次要写三四个文件（每张脏表各一次「临时文件 + fsync + 改名」），
而 `docs/contributing.md` 立了「输入优先于学习，为学习增加的延迟算设计错误」。代价是最坏丢不到一分钟。

- **随包资源目录（`filesDir/bundle/`）**：壳把 APK 的 assets 解到这儿，整个目录交给 Rust 当 `bundle`。
  里头现在有五样：emoji 字体与两张 emoji 表、表情面板两张表（见 `assets/emoji/README.md`）、
  英文词表 `english.tsv`、语言模型 `lm.qj`、释义表四本 `glossary-{zh,en,ja,es}.qj`，
  外加一个子目录 `dicts/` 放 11 本领域词库。
  **有哪张用哪张**，三种成色：
  - emoji 那几张**缺一张键盘就画不出表情**，所以壳那边「有一个解不出来就整个放弃」（`ensureExtras`）
  - 其余（英文词表、语言模型、释义表、领域词库）都**是可选的**，单独解、失败不拦
    （`required = false`，日志降一档）——少了它们英文模式退回直输、整句退化成一元词频、
    候选条不画译文、专业词查不到，不该把 emoji 一起拖下水。那份清单是
    `QingjianImeService.OPTIONAL_ASSETS`（领域词库那个子目录**有几本解几本**：
    `assets.list("dicts")` 问包里有啥，名字不写死）
  - Rust 侧读不到哪张就少哪块功能，**都不影响启动**；这条契约有
    `a_broken_bundle_still_opens_the_session` 守着（解包拷一半断了是真会发生的）
  - **领域词库那个子目录现在全开**（2026-09-22 用户定的）：把目录里几本的名字都算进
    `DictionariesConfig::domains`。桌面缺省只开 `idioms`（`DEFAULT_DOMAINS`，理由是
    「成语四字全拼几乎不歧义，收益稳；其余按需打开」），**等 E7 有了设置页再定安卓要不要回到那套**

  这个目录原来叫 `emoji`，2026-09-22 加英文词表时改的名（改名会让老安装重解一遍 emoji 字体，
  安卓还没发版，不管）。词库 `dict.qj` **不在这里**：它跟别的产品数据一样单独解到 `filesDir` 根上，
  路径由壳显式传给 `open`。
- **包体与首次唤起**（2026-09-22 实测）：`filesDir/bundle/` 现在装着约 118 MB
  （emoji 字体 10.7 + 词表 2.3 + 语言模型 44.4 + 释义表四本约 53 + 领域词库 11 本 7.4）。
  APK 那边 60 MB（x86_64 版）。这些是在 `onCreate` 里
  **同步**拷出来的，所以装完 / 升级后**第一次唤起要等**：进程起来到资产加载完约 **0.6 秒**
  （模拟器上量的，那时只多拷 44 MB 的模型；四本释义表加起来与它差不多量级），
  之后靠 `.名字.installed` 标记跳过。大件都是 mmap（语言模型 12 ms、释义表同样是查表），
  **加载本身不吃启动时间，吃的是那次拷贝**。
- **释义表**（2026-09-22，E4）：`glossary-{zh,en,ja,es}.qj` 四本，`zh` 是英→中
  （英文模式的候选用它，`with_english_translator`），另外三本是学习语言（`with_translator`）。
  **学习语言现在写死 `Language::English`**（`session/mod.rs` 的 `LEARNING_LANGUAGE`）——
  桌面那边是配置项 `[general] learning_language`，安卓还没有配置文件（E5 / E7），
  三本都随包带着，换表那步只是改这一个常量。填 `Row.annotation` 的拼法**照搬桌面**
  `apps/windows/server/src/ui/candidates/row.rs` 的 `from_candidate`（读音 → 每个义项
  「词性 + 译词」、义项间 ` · `）——那是同一套逻辑的两个副本，改一边记得看另一边。
- **英文模式两条路**（`Session::type_letter`）：`bundle` 里有 `english.tsv` 就 `with_english` 挂上，
  同时在 `Session` 里记一个 `english_candidates`，英文模式下字母进组句缓冲区（大小写按 Shift 定好
  再交给引擎，英文里大小写有意义），候选条出补全与拼错纠正；**没词表就退回直输**——
  字母不进缓冲区、直接打给应用，也就是桌面关掉 `[general] english_candidates` 时那条路。
  那个开关**要两个条件**：随包词表在（没表就压根没候选可给），且配置里 `[general] english_candidates` 开着。
- **配置（E7，2026-09-22 接上）**：落 `filesDir/config.toml`，与桌面**同一份格式、同一套缺省**
  （`qingjian-platform` 的 `Config`；第一次开会话时把带注释的模板写出来，用户从此有份能手改的配置）。
  路径只在 `settings::config_path` 一处拼，会话与设置页都走它。
  - **读**取整份 JSON（`{"ok":…}` 信封，壳用安卓自带的 `org.json` 解、零依赖），**写**逐键
    （`configSetBool/Int/String/Array`，返回空串 = 成功）。**值必须按类型分开写**——
    `set_value` 收的是 `impl Into<toml_edit::Value>`，把 `false` 当字符串写进去会变成 `"false"`，
    下次 `load` 直接解析失败。落盘仍走 `Config::set_value`，**注释与顺序都留着**。
  - **设置页只跟文件打交道，不碰会话句柄**：`handle` 是 `Box<Session>` 的裸指针，
    输入法服务随时可能在 `onDestroy` 里把它拆掉，设置页拿着就是野指针。改完也不用通知谁。
  - **生效时机只有一处：`onStartInputView`**（`Session::poll_config`）。用户从设置页回来键盘必然
    重弹一次，而这是**唯一一定到**的回调（BACK 收键盘时 `onFinishInput` 不来，E1 实测过）。
    桌面要每秒轮询是因为它没有「键盘弹出」这个事件；安卓有，就不该白养一个定时器——
    **打字那条路上一个字节都不加**。代价：手改配置文件要重弹一次键盘才生效。
  - 判「变了没有」**比文件原文，不看 mtime**：`write_atomic` 是「写临时文件 + 改名」，
    同一秒里改两次 mtime 可能相等，那一次就被吞了。顺带一个好处：坏文件只在原文真变了才重试。
  - **改坏了**：`load` 报错就记一条日志、**沿用上一份能用的**，绝不替用户「修好」
    （那会把他写的注释和顺序一起抹掉）；设置页那边把错误显示出来、控件按文件重读一遍。
  - **推给引擎的收在 `push_to_engine` 一处**（模糊音 / 繁体 / 全角标点 / 中文优先 / 双拼 / 学习开关）。
    `Session::open` 与 `apply_config` **两边都得调**——只在后者里设的话，启动读到的那份永远补不上
    （`poll_config` 见文件没变直接返回 0，2026-09-22 被测试抓到）。换释义表与重读词库**不在**里面：
    那两样要 mmap 文件，各自单独判「真变了没有」。
- **震动（`[keyboard]` 分节）**：`vibration`（`off` / `tick` / `click` / `heavy` / `double` / `custom`）
  加 `vibration_ms`。除 `custom` 外都走**系统的预置触感**（`VibrationEffect.createPredefined`）——
  厂商针对自家马达调过，而一个手填的毫秒数（K2 那个 20ms 是「试出来的起点」）在每台机器上
  表现都不一样；设备不支持时系统自己退回平台波形，所以不必查 `areEffectsSupported`。
  键名在 `qingjian-platform` 的 `VibrationStyle` 与 Kotlin 的 `VibrationStyle` 各有一份
  （跨语言没法共享），**改一边必须同时改另一边**。
  壳在 `onStartInputView` 里读一次配置交给 `KeyFeedback`，不是每敲一下读文件。
- **云联想（E8，2026-09-22 接上）**：`[predict] enabled` 且有密钥就 `engine.set_predictor`，
  失败只 `warn` 退回不联想（照 Windows 的 `attach_cloud`）。**结果是非阻塞取的**
  （提交立刻返回，答案得回来取），所以壳要轮询——但**只在真有请求在飞时**跑那个 50ms 的
  心跳（掩码里的 `flags::PREDICTING`），拿到结果或等够 12 秒就停。**没在等结果时一个定时器都不跑**。
  - **防抖不用壳操心**：它在 `qingjian-predict` 的 worker 里（`recv_timeout(debounce)`，
    缺省 300ms，只把最后一个真发出去），壳每次敲键都提交。
  - **两个只有真跑起来才现形的坑**（都 2026-09-22 在模拟器上抓的）：
    ① `poll_prediction` 在「还没回来」时**也得返回 `flags::PREDICTING`**——返回 0 的话壳
    以为不用再问了，结果永远收不到；② `relayout()` **不标脏**（标脏的是 `refresh()`），
    改完候选不调 `refresh` 界面就纹丝不动。
  - **隐私那道闸**：壳按 `EditorInfo.inputType` 判密码框 → `setPrivate` → 引擎的
    `Engine::set_private` 挡住**一切**外发（学习、输入日志、云联想都是）。进密码框时
    **已经发起的那一轮也作废**（`Session::set_private` 顺手清掉整句与云端词）。
  - **上下文**：`onStartInputView` 里取光标前后各 256 字符报上来，引擎按
    `lookback / lookahead` 再裁。Windows 那边传 `None`，安卓像 macOS 一样给。
  - **安卓上跑网络的几条硬规矩**（2026-09-22 在真机上一条条撞出来的，都是「电脑上好好的、
    手机上不行」）：
    ① **`http://` 缺省被系统拦**（Android 9 起），而电脑上那两个壳没有这条限制——同一个
       地址在电脑上连通、到手机上就失败，报错还看不出是它拦的。放行写在
       `app/src/main/res/xml/network_security_config.xml`。
    ② **别用 reqwest 缺省的 rustls 后端**：它要 `rustls-platform-verifier`，而那个在安卓上
       **必须先拿系统 `Context` 初始化**，不初始化就在握手前 panic
       （`expect rustls-platform-verifier to be initialized`）。那套初始化要在 Gradle 里加
       maven 仓库 + Kotlin 组件，对输入法太重——现在自己在 `chat_client::client_builder`
       里铺 `webpki-roots` 的静态根证书（`#[cfg(target_os = "android")]`，桌面不动）。
    ③ **Rust 的 panic 消息在安卓上会凭空消失**（stderr 没接到 logcat），所以跨线程的活儿
       都要拦一道 `catch_unwind` 把话捞出来——见 `qingjian-predict/src/panic.rs`。
       同类教训：`try_recv().ok()` 把「通道断了」和「还没到」混成一回事，界面就会一直转圈。
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
  键「还落在同一个键上」就一直算按着（键大，抖几像素不该掉字），候选条「挪出触摸阈值」才算没挪窝（横向拖是滑动选词的手势）；
  共用的 `within_slop` 在 `src/touch.rs`，阈值那点事只留一个版本。
- **取角标改成长按字母键弹一排选项**（2026-09-21，撤掉「键上滑动」）。
  长按够 300ms（壳的心跳来问 `Session::repeat` → `Keyboard::begin_choice`）弹出
  **`[大写][符号][小写]`** 三格，默认中间那个；手指左右滑过 `Chooser::STEP`（16 点）换一格，
  松手兑现所选。选中的那格在气泡里垫一块底色（`Popup::Choices`；`popup_for` 第四个数
  从 bool 改成「形态」编码：0 普通 / 1 松手清空 / 2 起是选中下标——手指滑一下就得重画）。
  三个选项各走哪条路见 `Chooser::fired`：大写走 `Literal`（中文模式下也直出大写字母）、
  小写走 `Letter`（照常进拼音）、符号走 `Literal`。
  **撤掉滑动取角标的原因**：真机上快打会误蹦符号，而那条路**治不好**——
  「手指出了键 = 作废」与阈值线重合（都在半个键宽上），把方向收成只认往下、阈值提到 22
  都只是压概率。长按是有意的动作，快打按不到 300ms，**误触面直接归零**。
  `SWIPE` 22 点现在只剩 ⌫ 上滑清空一处用。
- **认成手势的手指不再因「滑出键外」作废**：`hinted` / `cursor_started` / `clearing` 三个都排掉了；
  而且 `hinted` 认出来的**那一刻会把之前记下的 `sliding` 抹掉**——往下滑够 22 点之前手指可能
  已经出了键（键矮的屏上更明显），不抹的话 `refresh_pressed` 就不认这根手指，
  气泡与键帽按下态会在手势认出的那一刻**当场消失**。
  （`clearing` 那次是把它加进豁免名单，`hinted` 这次是回头抹掉——同一个坑的两副面孔。）
- **候选条上的长按不做任何事**（2026-09-20）：`Session::repeat` 只认键盘上的键（退格连发），
  候选条那条路整个没有。原先挂过「长按候选 = 删词」（K8），摘掉的理由见 `docs/plan/android-keyboard.md`。
  换句话说，候选条上按住不放就是「慢慢点一下」，松手照常选中那个词。
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
  拼音用 `setComposingText` 镜像（`takePreedit`）。**顺序是先上屏再镜像**：
  候选比输入短时（`kaifazhe` 选「开发」）剩下的拼音还在缓冲区里，镜像晚了就丢了。
- **拼音没了的时候，不能用 `finishComposingText()` 收尾**（2026-09-20 踩到，用户报「ni 打错了
  要按好几下退格才干净」）。它的语义是「组字到此为止，**文字留在原处**」——只去掉那层下划线，
  **一个字都不删**。拿它收尾，镜像过去的拼音就被**烘焙成了正式文本**：输入法这边缓冲区已经空了、
  应用那边却多出一串；用户按退格删掉的是输入法的拼音，那一串留在原地，于是要多按几下。
  撤掉要用 `setComposingText("", 0)`——把组字区**替换成空**，也就是删掉。
  壳里用一个 `mirrored` 记着「应用那边此刻有没有我们镜像过去的组字区」：
  镜像非空拼音时置上，上屏时 `commitText` 会把组字区一起换掉所以跟着清掉，
  拼音变空且 `mirrored` 还立着时才走撤掉那条路（不然会平白给应用造一个空的组字区）。
  受影响的路径有三条：退格删空、候选条上的 `×`、换应用时的 `clear()`——原先三条都会留下一串。
- **候选条是一条能横滚的带子**（2026-09-21；这天先做过一版「横滑选词」，用户说理解错了，换掉了）。
  手指在候选条上横着拖，带子**跟着手指平移**（`Session::scroll_by`，按**位移增量**加，所以是跟手的），
  **拖动只是看，不上屏**：要选还得点一下（或空格上屏最左边那个）。
  参考项目 fcitx5-android 的候选条**根本不滚**（`canScrollHorizontally=false`，满 `maxSpanCount` 就收、
  多的进展开面板），所以这条是照「看得见更多候选」的目标自己定的，不是抄来的。
- **整条候选铺开，滚到哪儿画哪儿**（2026-09-21 晚；用户报「滑了半天页码不动」）。
  `Renderer::bar_strip` 一次把**全部**候选铺开（`renderer/bar/strip.rs` 的 `BarStrip`：每格的左边、格宽、
  整条多长、视口能看到哪一段、一共几屏），`Session::scroll` 是**整条带子的位移**，画哪几格由
  `BarStrip::slice` 现算（一屏十来格，比原先一次铺 24 个还少画些）。
  - **格宽只算一份**（`Renderer::bar_cell_widths`）——会话不自己再算一遍，否则画出来的位置与算出来的
    页码会各走各的。
  - 喂给 `render_bar` 的 `scroll` 要用 `BarStrip::local_scroll` 折成「第一格相对视口左缘」：
    渲染是从**传进去的第一格**开始往外铺的，不是从整条的开头。
  - 命中矩形里的候选下标也是「画出来这一批里的第几个」，`Act::CommitCandidate` 要加上
    `visible().start` 才换回整份候选表——**错一格就上屏隔壁的词**，测试
    `tapping_after_scrolling_commits_the_one_under_the_finger` 盯着这条。
  - 代价：每次组句变化要量**全部**候选的宽度。实测 500 个约 1.2 ms（release、桌面），只算一次，可以接受。
- **页码 = 第几屏 / 共几屏**，跟着手指走。分母是「能停在几个位置上」（`floor(最远位移 ÷ 屏宽) + 1`），
  **不是**「带子有几屏长」——不然翻到底时分子会停在 24/25 那种数上，走不到分母。
  带子一屏就装得下时不报页码，那两个 `‹ ›` 箭头跟着不画。`‹ ›` 一次滚一屏（折到屏边界上再挪，
  在半个屏的地方按一下也该走到下一屏的边界）。
  先前那版是「一次铺 24 个（`WINDOW`）、一批批翻」，页码说的是**第几批**：一屏看得见十来个，
  要滑满三屏多才跳一格，所以看着像坏了。**「批次」连同 `WINDOW` 一起撤了**——`scroll_by` 现在只剩
  「加上位移、两头夹住」，带子是通的。更早那版还要从**上一帧的命中矩形**量页宽（`page_width`），
  差一帧、还得**先量再加**（踩过）；现在位置是铺开时一次算好的，这一类坑没有了。
- **甩一下松手，带子自己滑一段**（2026-09-21，`session/fling.rs`）：分工照旧「节拍在壳、手感在 Rust」。
  - **速度由壳量**：安卓自带 `VelocityTracker`（`QingjianSurfaceView` 里 `obtain` 一个，
    抬手前 `computeCurrentVelocity(1000)` 再取 `getXVelocity(pointer)`——**要先量再加这一笔**，
    抬手那一笔一进 tracker，那根手指的历史就被清了、再取只会拿到 0）。单位是**像素/秒、向右为正**，
    Rust 那边除以 1000、取负（手指往左甩 = 带子往后滚，与 `scroll_by` 的正方向一致）。
  - **只有刚才真的滚过这条带子的那根手指才算数**（`Session::scrolled`）：抬手时每根手指壳都会报速度上来，
    点候选、敲键盘也报，照单全收去滑带子就成了乱动。
  - **衰减是每毫秒乘 0.998**（一秒后剩一成多），总滑行距离 ≈ 起手速度 ÷ 0.002——轻甩四五百点、重甩两屏。
    位移按**积分**算（`v₀·(k^dt − 1)/ln k`）而不是 `v·dt`：那样帧率一变滑的距离就变了，
    而壳那一拍实际过去多久（卡一下可能 30ms）是不定的。有测试盯着「8ms 一帧与 32ms 一帧滑得差不多」。
  - **起手低于 200 像素/秒不算甩**（慢慢拖到一半松手不该自己跑）；慢到 50 像素/秒就停；
    手指一落下、组句一变、尺寸一变，滑行都停。
  - 帧由**视图**排（`flingTicker`，16ms 一条 `Runnable`），**不动连发那个 50ms 的心跳**；
    掩码里那个 `flags::FLING`（16，唯一一个「下一步该做什么」而不是「哪个面变了」的位）
    就是「还在跑，接着敲帧」的信号，滑完那一拍自然就不带了，壳跟着停。
  - 一帧的实际过去时间**夹在 [1, 48] 毫秒**：真卡了半秒的话照实算会把带子一下甩出去老远。
  - **开销实测**（模拟器、release）：一次甩动约 150 帧，只有 1 帧超过 16ms（22ms）。
    所以「复用同一张 Bitmap、别每帧新建」那条优化**先不做**——真机上要是掉帧再来。
- **高亮就是最左边那个「整格」**（`BarStrip::first_whole`）：空格上屏与「组句中打标点先上屏高亮候选」
  都走 `highlighted_candidate`，**「高亮在哪」只有一个说法**。带子一滚高亮跟着滚——停在原处的话，
  滚出去之后空格上屏的会是个屏幕上根本看不见的词（2026-09-21 之前高亮是「本页第一个」，不跟着滚）。
  **认整格不认「露了半边的那个」**：滚到格子中间时最左边那格只露一条边，压在上面看着像画坏了，
  空格上屏的也会是个几乎看不见的词。
- **换应用时丢掉没上屏的拼音**：`onFinishInput` 里清一次，免得在 A 应用敲的拼音跑到 B 应用里。
- **中 / 英切换**（2026-09-18 起是「英文直输」，2026-09-22 随英文词表接上改成有候选）：
  见上面「英文模式两条路」。标点始终半角。
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
- **「一页画几个」这套已经没有了**（2026-09-21）：按音节数定页大小（2026-09-20）→ 一次铺 24 个的
  `WINDOW` → 整条铺开、画哪几格由滚动位置现算，前后三版都撤了，见上面「整条候选铺开」那条。
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
- **一台模拟器当几台手机用**（2026-09-20 验键盘高度自适应时用的）：`adb shell wm size 720x1920`
  直接换逻辑屏幕尺寸（密度不变，所以点数是 `尺寸 ÷ 密度`），`wm size reset` 还原。
  验「随屏幕变的东西」不必建好几个 AVD。
  **量画出来的东西有多大时别拿 `dumpsys window` 的 frame 当准**：横屏下输入法窗口的 frame
  比实际画出来的键盘高出约 97 点（窗口上部一条透明带，应用从底下透出来），照它算键盘高度会错。
  竖屏两者倒是严丝合缝。可靠的办法是从截图像素里找边界，几个颜色照着对：
  键盘底色 `(220,224,228)`、功能键 `(188,192,204)`、内容键 `(252,252,252)`、设置页应用底色 `(238,237,244)`。
  **逐行统计「应用底色占这一行的多少」**，找最后一条几乎整行都是应用色的行，它下面就是键盘顶边。
  （踩过：只扫左边缘某一列是不行的——那一列会穿过键帽和 `⇧` / `符` 这些功能键，
  量到的是键的边界不是键盘的边界，四张图会给出同一个假答案。）

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
