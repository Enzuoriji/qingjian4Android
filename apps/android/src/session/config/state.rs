//! 会话记着的那一份配置。

use std::path::{Path, PathBuf};

use qingjian_core::Language;
use qingjian_platform::{Config, DictionariesConfig};

use crate::settings::config_path;

/// 会话这一份配置：从哪儿读、上次读到的原文、当前生效的那一份。
///
/// **拿文件原文比，不看 mtime**：`write_atomic` 是「写临时文件 + 改名」，同一秒里改两次
/// mtime 可能相等，那一次改动就被吞了。文件才几 KB，整份读来比一次 stat 贵不到哪去。
/// 顺带一个好处：**坏文件只在原文真变了才重试**，不会每次去啃同一个解析不了的文件。
#[derive(Debug)]
pub(crate) struct ConfigState {
    /// 配置文件路径。`None` = 壳没给可写数据目录，一切按缺省、也不落盘。
    path: Option<PathBuf>,

    /// 上次读到的文件原文。`None` 表示那时读不到（不存在或读不动）。
    text: Option<String>,

    /// 当前生效的配置。**解析失败时保持上一份能用的**——配置读不了不该影响打字。
    config: Config,

    /// 上次解析失败的原因，只给日志和排查用。
    error: Option<String>,

    /// 已经挂上的学习语言（`None` = 没挂释义表，也就是不显示译文）。
    ///
    /// 换一本释义表要 mmap 一份十几 MB 的 `.qj`，所以**只在它真变了才做**。
    language: Option<Language>,

    /// 已经挂上的领域词库开关，同样只在真变了才重读那几本词库。
    dictionaries: DictionariesConfig,
}

impl ConfigState {
    /// 按数据目录开一份状态：先把带注释的模板写出来（文件在就不动它），再读一次。
    ///
    /// **模板这一步是为了让安卓也有一份能手改的配置**——与桌面两壳同一个 `TEMPLATE`。
    pub(crate) fn load(data_dir: Option<&Path>) -> Self {
        let mut state = Self {
            path: data_dir.map(config_path),
            text: None,
            config: Config::default(),
            error: None,
            language: None,
            dictionaries: DictionariesConfig::default(),
        };
        let Some(path) = state.path.clone() else {
            return state;
        };
        match Config::write_template_if_missing(&path) {
            Ok(true) => tracing::info!(path = %path.display(), "写出了缺省配置"),
            Ok(false) => {}
            Err(error) => tracing::warn!(path = %path.display(), %error, "配置模板写不出来"),
        }
        state.refresh();
        state
    }

    /// 当前生效的配置。
    pub(crate) fn config(&self) -> &Config {
        &self.config
    }

    /// 已经挂上的学习语言。
    pub(crate) fn language(&self) -> Option<Language> {
        self.language
    }

    /// 记下这次挂上了哪本释义表。
    pub(crate) fn set_language(&mut self, language: Option<Language>) {
        self.language = language;
    }

    /// 记下这次的领域词库开关。
    pub(crate) fn set_dictionaries(&mut self, dictionaries: DictionariesConfig) {
        self.dictionaries = dictionaries;
    }

    /// 已经挂上的领域词库开关。
    pub(crate) fn dictionaries(&self) -> &DictionariesConfig {
        &self.dictionaries
    }

    /// 文件变了吗；变了就重读。返回 `true` 表示**配置真的换了**（只是注释变了不算）。
    ///
    /// 读不到文件（不存在、被删了）时**退回缺省**——这与桌面一致：
    /// 配置是唯一事实源，文件没了就是没配置。
    pub(crate) fn refresh(&mut self) -> bool {
        let Some(path) = self.path.clone() else {
            return false;
        };
        let text = std::fs::read_to_string(&path).ok();
        if text == self.text {
            return false;
        }
        let changed = match Config::load(&path) {
            Ok(config) if config == self.config => false,
            Ok(config) => {
                tracing::info!(path = %path.display(), "配置已更新");
                self.config = config;
                self.error = None;
                true
            }
            Err(error) => {
                // 不替用户「修好」文件（桌面明确不修），也不能让笔误把当前这份弄丢。
                // 原文记下来是为了**同一份坏文件不反复重试**，改好了下次照样读。
                tracing::warn!(path = %path.display(), %error, "配置读不了，沿用上一份");
                self.error = Some(error.to_string());
                false
            }
        };
        self.text = text;
        changed
    }

    /// 上次解析失败的原因（没有就是 `None`）。
    #[cfg(test)]
    pub(crate) fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
}
