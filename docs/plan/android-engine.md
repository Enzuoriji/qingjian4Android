# 安卓：接引擎（数据与能力）

2026-09-18 定。安卓壳目前只把引擎**最基本的那条路**接上了；桌面壳接了一整套数据与能力。
这份排的是「把差的那几样补齐」，按性价比排序。键盘那一侧见 [android-keyboard.md](android-keyboard.md)。

**工期口径**：半天 / 一天，指「写完 + 在设备上验过 + 有自动化测试」的工作量，按这一轮的节奏估
（中等功能（含设备验证与测试）半天到一天）。不是日历天。

## 现状（有据可查）

- **词库不是问题**：安卓装的 `.qj` 与产品数据里的 `data/generated/dict.qj` **候选逐行相同**，
  只是 `.qj` 元数据（名称 / 许可 / 署名）不一样。对照方法与结论见下面「已查证」。
- **问题是配套没接**：`Session::open`（`apps/android/src/session/mod.rs`）调了六样——
  词库、emoji 表（`with_emoji`）、emoji 字体、**用户学习（`with_learner`）**、
  **英文词表（`with_english`）**、**语言模型（`with_language_model`）**（后三样都是 2026-09-22 接上的）。
- 桌面（`apps/macos/src/host/init.rs`）调了：`with_learner`、`with_translator`、`with_english_translator`、
  `with_english`、`with_emoji`、`with_language_model`，另有本地小模型 `set_async_sentence_scorer` 与整份配置。

### 对照表

| 接进引擎的东西 | 桌面 | 安卓 | 少了会怎样 | 体积 |
|---|---|---|---|---|
| 词库 | ✅ | ✅ | — | 3.5 MB |
| emoji 表 + 字体 | ✅ | ✅ | — | 0.2 + 10.7 MB |
| 用户学习（落盘） | ✅ | ✅ **2026-09-22** | — | 小 |
| 语言模型 `lm.qj` | ✅ | ✅ **2026-09-22** | — | 44 MB（进包约 18 MB） |
| 英文词表 | ✅ | ✅ **2026-09-22** | — | 2.3 MB |
| **释义表 `glossary-*.qj`** | ✅ | ❌ | 候选旁没有译文与词性（README 的招牌） | 3–22 MB |
| **领域词库 11 本** | ✅ | ❌ | 专业词查不到 | 7.4 MB |
| **配置（模糊音等）** | ✅ | ❌ | 口音适配、每页候选数、自定义短语都没有 | — |
| 本地整句小模型 `model.qjm` | ✅ | ❌ | 「停顿一下由小模型重排候选」没有了 | 54 MB |
| 云联想 | ✅ | ❌ | 云端整句补全没有了 | — |

### 已查证（2026-09-18）

```bash
tools/release/data-fetch.sh        # 拉产品数据到 data/generated/
RUST_LOG=error cargo run --release -q -p qingjian-cli -- --dict data/generated/dict.qj nihao woshi weishenme womendedajia > a.txt
RUST_LOG=error cargo run --release -q -p qingjian-cli -- --dict assets/lexicon/dict.tsv  nihao woshi weishenme womendedajia > b.txt
diff a.txt b.txt                   # 只差耗时行，候选完全相同
```

语言模型单独 A/B 过（把 `lm.qj` 挪走再跑）：`nihaoma` / `womenshizhongguoren` / `jintiantianqihenhao`
**没有模型时整句也是对的**——一元词频兜底够用。所以模型值不值得带 43 MB，**要在一个更长的句子上再验一次**，
别凭「有总比没有好」就加。

## 排期

| 组 | 内容 | 工期 | 累计 |
|---|---|---|---|
| **一** | 学习落盘 + 英文词表 + 语言模型 | 1.5 天 | 1.5 天 |
| **二** | 释义表与译文 | 2 天 | 3.5 天 |
| **三** | 领域词库 + 配置 | 1.5 天 | 5 天 |
| **四（酌情）** | 云联想 / 本地小模型 | 各 2–4 天 | — |

---

## 一、把「会学习」和「有词」补上（1.5 天）

### E1 用户学习落盘 —— 半天

**做什么**：`Session::open` 接 `with_learner`，数据落 `filesDir/learning/`。
范本：`apps/macos/src/host/init.rs` 的 `load_learner(dir)`（`FrequencyLearner::from_path`，
读不出来就 `default()`）。

**为什么排第一**：改动最小、对「越用越顺」的体感最直接。现在安卓**一重启就忘光**——
而输入法进程在安卓上被杀得很频繁。

**验收**：选一次「你好」，杀掉输入法进程再唤起，同一个拼音里「你好」的名次提前（写成宿主机测试，
用临时目录）。

**注意**：写完要 flush；进程被杀时不能丢已经学到的（看 `FrequencyLearner` 有没有落盘时机，
必要的话选中就写）。

**2026-09-22 已做。** 做法与上面几处出入，都以实测为准：

- 数据落 `filesDir/learning/`（主文件 `user.tsv`）。**`open` 里得先 `create_dir_all`**——
  `write_atomic` 只写文件、不建父目录，省了会一路静默失败（只有一条 warn，外面看不出没存上）。
- **不是「选中就写」**，是**「键盘窗口藏起来就写」**：一次落盘要写三四个文件，
  而 `docs/contributing.md` 立了「输入优先于学习」。详情与四个时机见
  `docs/notes/crate-notes.md` 的 `apps/android` 那节。
- **验收目标词不是「你好」**：它在 `nihao` 下本来就是第一名，学不学都看不出来。
  换成「你好好」也不行——那个靠简拼拆出来，而排序键里「音节省略」「完整匹配」排在
  「这个输入串下选过没有」**前面**，选多少次都换不了位。最后用 `kaif` 的「开发 / 开放」
  （都是 `kai` + 简拼 `f`，结构上并列），一次就换位。
- 验收两条都走了：宿主机 `the_learner_survives_a_restart`（临时目录建会话 → 选词 → 落盘 →
  重开 → 断言名次提前）；模拟器上 `kaif` 选「开放」→ 收起键盘（**同秒落盘**）→
  `force-stop` 杀进程 → 切回青简重打 `kaif`，**「开放」已升到第一位**。
  截图见 `screenshots/2026-09-22_学习落盘_*.png`。

**实测多踩的一个坑**：`onFinishInput` **在按 BACK 收起键盘时不触发**（`ImeTracker` 只报
`HIDE_SOFT_INPUT_BY_BACK_KEY`），数据当时是等 60 秒心跳兜下来的。补 `onWindowHidden`
才做到「收起来就写」。这条已写进 `crate-notes.md`。

**顺带一条给真机验收的提醒**：`adb shell am force-stop` 杀输入法会把系统默认输入法
**弹回 Gboard**，重测前要 `ime set` 切回青简。

### E2 英文词表 —— 半天

**做什么**：`with_english(WordList::from_path(...))`，把 `english.tsv`（3 MB）打进包。

**为什么**：现在英文模式是**直输**（字母直接打出去、没有候选），就是因为没有这张表。
接上之后英文模式才有补全与拼错纠正。

**验收**：英文模式敲 `comp` 出 `Company` / `Compare`（现在只能得到字面的 `comp`）。

**2026-09-22 已做。** 实况：

- 词表跟词库一样由 `build.sh` 拷进 APK 的 assets（优先 `assets/lexicon/english.tsv`，
  退回 `data/generated/english.tsv`，与 mac 的 `bundle.sh` 同序），**没有也不算错**——
  英文模式退回直输，只是少一块功能。
- 解包落在随包资源目录里（那个目录原来叫 `emoji`，现在叫 `bundle`：里面已经有 emoji、颜文字、
  英文词表三样了）。**它是可选的**：`ensureExtras` 里 emoji 那几张是「缺一张就整个放弃」，
  英文词表单独解、失败不拦，免得把键盘画不出来一起拖下水。
- 壳侧连带改了 `type_letter`：有词表时字母进组句缓冲区（大小写按 Shift 定好再给引擎，
  英文里大小写有意义），没词表时保持原样直输。`Session` 里记一个 `english_candidates`——
  桌面那边这是配置项 `[general] english_candidates`，安卓还没有配置文件（E7），先按「词表在不在」定。
- **验收的措辞要改**：计划里写的「出 `Company` / `Compare`」是照桌面（敲大写 `Comp` 时引擎
  把首字母改回去）写的。实测**敲小写 `comp` 出的是 `company` / `companies` / `complete`**，
  与词表里存的小写一致（`adapt_case` 认大小写）。`compare` 在词表里（词频 4450）但排在
  那三个后面，敲到 `compa` 才露出来。
- 验收两条都走了：宿主机两条新测试（临时目录里手写一张小词表 → 敲 `comp` 出 `Company`/`Compare`；
  以及没词表时仍是直输）；模拟器上切英文模式敲 `comp` → 候选条 `comp | company | companies |
  complete`，再敲 `a` 收窄成 `company | companies | compared | comparison`，点一条上屏。
  截图见 `screenshots/2026-09-22_英文候选_*.png`。

### E3 语言模型 —— 半天（+43 MB 包体）

**做什么**：`with_language_model(BigramModel::from_path(lm.qj))`，把 `lm.qj` 打进包。

**先决**：**先验收益**。上面 A/B 显示一元兜底在短句上已经够用；拿 3–5 个长句、口语句再比一次，
收益不明显就先不带（43 MB 不是小数）。

**验收**：同一批句子，带模型与不带模型的首选对比，写进这份文档。

**2026-09-22 量过了，结论是带上。** 用 `apps/cli --eval-text` 跑一份 24 句的评测集
（12 段长句 / 口语句，按标点切成 24 句，冻结在 `data/eval/sentences.tsv`，两次跑的是同一份）：

| | 首选命中 | 整句候选 | 字准确率 | 查询平均 |
|---|---|---|---|---|
| **带 `lm.qj`** | **62.5%** | 62.5% | **95.0%** | 1.6 ms |
| 不带 | 37.5% | 37.5% | 88.8% | 1.4 ms |

**首选差 25 个百分点**（相对提升 67%），而且 24 句里**没有一句是带模型反而变差的**。
错的那些全是同音词：「在想想」→「在想象」、「下结论」→「下杰伦」、「写作业」→「带回家写作业」、
「降雨过程」→「本事将出现」——正是二元模型该治的病。

**当初「一元兜底够用」那个结论只适用于短句**：那时比的是 `nihaoma` / `womenshizhongguoren` /
`jintiantianqihenhao`，句子短到词级词频就够拍板了。长句上一元完全不成立。

代价那一侧（2026-09-22 模拟器实测）：

- **包体**：APK 28 MB → **46 MB**（`lm.qj` 44 MB，进包压到约 18 MB）
- **加载**：**12 毫秒**（mmap，不吃启动时间）
- **查询**：平均 1.6 ms vs 1.4 ms，可以忽略
- **首次唤起的解包**：进程起来到资产加载完 **0.6 秒**（这一段要把 44 MB 从 APK 拷到私有目录，
  一次性，装完/升级后第一次唤起才付）
- **落点**：`filesDir/bundle/lm.qj`，与 emoji、英文词表同一个随包资源目录

**接法与取舍**：与英文词表同一套——`build.sh` 拷进 APK 的 assets（源是 `data/generated/lm.qj`），
壳解到随包资源目录，Rust 在 `Session::open` 里 `with_language_model`。**读不出来只记日志**
（整句退化成一元词频），也不像 emoji 那样「缺一张就整个放弃」——它是可选的，
不该把键盘画不出来一起拖下水。契约有 `a_broken_bundle_still_opens_the_session` 守着。

**没做的**：`lm-unigram.tsv` + `lm-bigram.tsv` 那条退路（CLI 有，安卓没接）——
包里带的是打好的 `.qj`，那两个 TSV 只有造模型时才用得上。

---

## 二、把译文画出来（2 天）

### E4 释义表 + 渲染器画译文 —— 1.5 天

**做什么**：
1. 加载 `glossary-<语言>.qj`（`with_translator`）+ `glossary-zh.qj`（`with_english_translator`）
2. `Session::recompose` 里调 `engine.annotate(&mut candidates)`——现在**故意没调**
3. 渲染器 `render_bar` 把 `Row.annotation` 画出来——现在只画序号与词
   （数据已经在 `Row.annotation` 里，`apps/windows/server/src/ui/candidates/row.rs` 有现成的
   `from_candidate` 可以照搬）

**为什么**：这是 README 开头的招牌（「候选旁附英文译文与词性」），也是「学语言」这条产品主线。
现在安卓上只有光秃秃的词。

**验收**：
- 宿主机测试：候选行的 `annotation` 非空，且第一段是词性 + 译词
- 预览 PNG：候选条上译文比词浅、比词性深（与桌面一致）
- 设备截图

**注意**：候选条一格只有 121 点宽（5 格均分），**译文多半放不下**。要么候选行改成两行
（词一行、译文一行），要么只在译文放得下时画。这是这一步的主要设计问题，**动手前先定**。

### E5 学习语言设置 —— 半天

**做什么**：`[general] learning_language`（en / ja / es / off），先给个最简单的入口
（设置写死在文件里也行，界面上先不做）。

**为什么**：E4 要挑加载哪张释义表；`off` 就是「不显示译文」——桌面有这个开关。

---

## 三、词库与配置（1.5 天）

### E6 领域词库 —— 半天

**做什么**：把 `data/generated/dicts/*.qj`（11 本，7.4 MB）打进包，用 `extra_dictionaries` 挂上。
范本：`crates/qingjian-platform` 的 `extra_dictionaries`。

**验收**：专业词（比如医学、法律）能查出来。

### E7 配置 —— 1 天

**做什么**：接 `qingjian-platform::Config`（TOML），至少让这几项生效：
模糊音 `[fuzzy]`、每页候选数 `[general] page_size`、自定义短语 `set_custom_phrases`。

**为什么**：安卓现在**一行配置都不读**。模糊音对南方口音是刚需（z/zh、n/l 不分）。

**验收**：开 `z-zh` 后 `zhongguo` 与 `zongguo` 都出「中国」。

**注意**：安卓上没有「偏好设置」界面，配置文件从哪来？（推一个？应用内放一个编辑入口？）
先定这个，再动手。

---

## 四、酌情（各自独立，先不做）

- **E8 云联想**（2 天）：接 `qingjian-predict`，要 API key、要网络权限、要隐私说明。
  桌面缺省就是关的，安卓先不做不亏。
- **E9 本地整句小模型**（3–5 天）：`model.qjm` 54 MB + candle 运行时。
  **包体和编译复杂度都上一个台阶**，除非前三组做完还觉得整句不行，否则不做。

---

## 顺带修（10 分钟，与工期无关）

安卓构建脚本给词库写死的许可是 `GPL-3.0-or-later`（`scripts/build.sh` 的 `pack_dict`），
但产品数据里写的是 `MIT AND Unicode-3.0` + 来源署名（`apps/macos/scripts/bundle.sh` 里那份是对的）。
**包里声明错了许可**，照 mac 那份改过来，署名也别丢。

## 风险

1. **包体**：**2026-09-22 实际走下来**：26 → 28（英文词表）→ **46 MB**（语言模型）。
   E4 的释义表按语言 3–22 MB 一本，加完还会再涨一截——`glossary-zh` 2.9 MB 是必带的，
   学习语言那本（en 15.6 / ja 21.8 / es 13.2 MB）要定清楚是随包还是按需下载。
2. ~~**E3 的收益没验**~~ **已验（2026-09-22）**：长句首选命中 37.5% → 62.5%，值这 44 MB，见上面 E3。
3. **E4 的排版**：候选条一格放不下译文，可能要改成两行——**这会动候选条的高度**，
   而高度是「敲一个键不会顶动应用内容」这条设计的前提，改之前先想清楚。
4. **E7 的配置来源**：安卓没有设置界面，配置从哪来是个产品问题，不是技术问题。
