//! 配置：读进来、推给引擎、以及换了一份之后重新应用。
//!
//! **配置文件是唯一事实源**（与桌面两壳同一条规矩）：设置页只管写 `config.toml`，
//! 这边负责读回来、让改动生效。设置页因此不必持有会话句柄，会话也不必知道有个设置页。
//!
//! 检查时机见 [`Session::poll_config`]——**只在键盘弹出时看一次**，不做常驻心跳。

mod state;

pub(super) use state::ConfigState;

use qingjian_core::{Engine, Language, NoTranslator};
use qingjian_platform::{Config, DictionariesConfig, GeneralConfig, extra_dictionaries};
use qingjian_translate::Glossary;

use super::{DICTS_DIR, Session};

/// 把配置里那几样**能当场推给引擎、代价 O(1)** 的推过去。
///
/// [`Session::open`] 与 [`Session::apply_config`] 都走它，**两边必须同一套**：
/// 只在 `apply_config` 里设的话，启动读到的那一份就白读了——`poll_config` 见文件没变
/// 直接返回，永远不会补设（这条 2026-09-22 被 `fuzzy_rules_from_the_config_reach_the_engine` 抓到过）。
///
/// 换释义表与重读领域词库**不在这儿**：那两样要 mmap 文件，各自单独判「真变了没有」。
pub(super) fn push_to_engine(engine: &mut Engine, config: &Config) {
    let general = &config.general;
    engine.set_fuzzy(config.fuzzy);
    engine.set_traditional_mode(general.traditional);
    engine.set_full_width_punctuation(general.full_width_punctuation);
    engine.set_chinese_first(general.chinese_first);
    engine.set_shuangpin(general.shuangpin());
    engine.set_learning(general.learning);
}

/// 配置里写的学习语言。
///
/// `off` 是「不显示译文」；写了个认不出来的代码（`de` 这种）**按英文**——
/// 与 [`qingjian_platform::GeneralConfig::shuangpin`] 认得不对就退回全拼同一条规矩，
/// 用户想学门语言、只是代码写错了，给缺省比给「什么都没有」强。
pub(super) fn configured_language(general: &GeneralConfig) -> Option<Language> {
    if general.learning_language_off() {
        return None;
    }
    match general.learning_language.parse::<Language>() {
        Ok(language) => Some(language),
        Err(_) => {
            tracing::warn!(
                value = %general.learning_language,
                "认不出的学习语言，按英文"
            );
            Some(Language::English)
        }
    }
}

impl Session {
    /// 配置文件变了没有；变了就重读并应用，返回 [`flags`] 的位掩码。
    ///
    /// **壳只在键盘弹出来时调（`onStartInputView`）**，不做常驻心跳：
    /// 用户从设置页回来时键盘必然重新弹出一次，这一条就够；而它还是唯一一定到的回调
    /// （BACK 收起键盘时 `onFinishInput` 不触发，E1 实测过）。桌面要每秒轮询，
    /// 是因为它没有「键盘弹出」这个事件——安卓有，就不该白养一个定时器。
    ///
    /// 代价是**手改配置文件要重弹一次键盘才生效**。将来真要支持即时生效，
    /// 再照 macOS 那样加一条 1 秒的轮询（`apps/macos/src/host/config/mod.rs` 的 `tick`）。
    pub fn poll_config(&mut self) -> i32 {
        if !self.config.refresh() {
            // 没变：壳那边**一个动作都不该有**。重画一张位图是几百微秒，白花。
            return 0;
        }
        self.apply_config()
    }

    /// 把当前这份配置推给引擎，返回 [`flags`] 的位掩码。
    ///
    /// 只推安卓真的用得上、且能当场改的那几样——桌面上那些外观 / 翻页键 / 快捷键项
    /// 在安卓没有对应物（候选条是通栏带子、键盘是自绘的固定布局），设置页也不摆。
    fn apply_config(&mut self) -> i32 {
        let config = self.config.config().clone();
        let general = &config.general;

        // 学习语言：这里唯一要 mmap 一份十几 MB 的动作，所以只在真变了才做
        let language = configured_language(general);
        if language != self.config.language() {
            self.swap_learning_language(language);
        }
        // 英文候选：随包词表在不在是前提（没表就没候选可给），配置说关就关
        self.english_candidates = self.english_words && config.general.english_candidates;

        // 领域词库：重读一次要 mmap 好几本 `.qj`，同样只在开关真变了才做
        if config.dictionaries != *self.config.dictionaries() {
            self.reload_dictionaries(&config.dictionaries);
        }

        push_to_engine(&mut self.engine, &config);

        // 缓冲区里那些候选是按老规则算出来的，重排一次
        self.recompose();
        self.mask()
    }

    /// 换学习语言的释义表。`None` 是「关掉，不显示译文」。
    ///
    /// 换失败保持原样（下次还试），只记日志——**少一块功能不能把打字拖下水**，
    /// 与 `open` 里那几处同一个取舍。
    fn swap_learning_language(&mut self, language: Option<Language>) {
        let language = match language {
            Some(language) => language,
            None => {
                if self.annotations {
                    self.engine.set_translator(Box::new(NoTranslator));
                    tracing::info!("学习语言已关，候选条不画译文");
                }
                self.annotations = false;
                self.config.set_language(None);
                // 那行小字没了，候选条要矮一截——位图得重画
                self.bar_dirty = true;
                return;
            }
        };
        let Some(bundle) = self.bundle.clone() else {
            tracing::warn!("没有随包资源目录，换不了释义表");
            return;
        };
        let path = bundle.join(format!("glossary-{}.qj", language.code()));
        match Glossary::from_path(language, &path) {
            Ok(glossary) => {
                tracing::info!(
                    path = %path.display(),
                    language = language.code(),
                    "释义表已切换"
                );
                self.engine.set_translator(Box::new(glossary));
                self.annotations = true;
                self.config.set_language(Some(language));
                // 高度跟着 `annotations` 走（off ↔ 非 off 差一行），位图必须重画
                self.bar_dirty = true;
            }
            Err(error) => {
                tracing::warn!(path = %path.display(), %error, "释义表换不了，沿用原来那本");
            }
        }
    }

    /// 重读领域词库。**只在开关真变了才调**——读一次要 mmap 好几本 `.qj`。
    fn reload_dictionaries(&mut self, dictionaries: &DictionariesConfig) {
        let Some(bundle) = self.bundle.as_deref() else {
            return;
        };
        let dir = bundle.join(DICTS_DIR);
        let loaded = extra_dictionaries::load(Some(&dir), None, dictionaries);
        tracing::info!(books = loaded.len(), "领域词库已重读");
        self.engine.set_extra_dictionaries(loaded);
        self.config.set_dictionaries(dictionaries.clone());
    }
}
