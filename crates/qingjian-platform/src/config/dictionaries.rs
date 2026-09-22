use serde::{Deserialize, Serialize};

/// 配置文件 `[dictionaries]` 分节：附加词库的开关。
///
/// 两类附加词库：随包的领域词库（`.app` 里 `Resources/dicts/`，法律 / 医学 / 地名 …）列在 `domains` 里的才加载
/// （缺省桌面只开 `idioms`、安卓全开，见 [`DEFAULT_DOMAINS`]）；
/// 用户目录 `dicts/` 下的 `.qj`（自己导入的）文件在就加载，只有列在 `disabled` 里的（按文件名，不含扩展名）跳过；
/// 导入 / 移除就是加 / 删文件。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DictionariesConfig {
    /// 打开的随包领域词库（文件名，不含 `.qj`）。
    pub domains: Vec<String>,

    /// 关掉的用户词库（文件名，不含 `.qj`）。
    pub disabled: Vec<String>,
}

/// 缺省打开的随包领域词库（桌面）：成语四字全拼几乎不歧义，收益稳；其余按需打开。
pub const DEFAULT_DOMAINS: [&str; 1] = ["idioms"];

/// 随包带的全部领域词库，按文件名排（与 `data/generated/dicts/` 里那 11 本一致）。
/// 安卓的缺省全开用得上，设置界面也照这个顺序列。
pub const ALL_DOMAINS: [&str; 11] = [
    "animals",
    "automotive",
    "finance",
    "food",
    "historical_figures",
    "idioms",
    "it_computing",
    "law",
    "medicine",
    "places",
    "poetry_lines",
];

/// 缺省打开哪几本。
///
/// **安卓全开**（2026-09-22 用户定的）：候选条是能横滑的带子、候选多不挤，而专业词
/// 「查得到」比「候选干净」重要；设置页上可以逐本关掉。桌面仍只开 `idioms`——
/// 候选窗是竖排的一页页翻，多开的每一本都在挤候选。
#[cfg(target_os = "android")]
fn default_domains() -> Vec<String> {
    ALL_DOMAINS.iter().map(|s| (*s).to_owned()).collect()
}

/// 缺省打开哪几本，见 [`DEFAULT_DOMAINS`]。
#[cfg(not(target_os = "android"))]
fn default_domains() -> Vec<String> {
    DEFAULT_DOMAINS.iter().map(|s| (*s).to_owned()).collect()
}

impl Default for DictionariesConfig {
    fn default() -> Self {
        Self {
            domains: default_domains(),
            disabled: Vec::new(),
        }
    }
}

impl DictionariesConfig {
    /// 用户目录里的词库是否启用。
    pub fn is_enabled(&self, stem: &str) -> bool {
        !self.disabled.iter().any(|d| d == stem)
    }

    /// 随包领域词库是否启用。
    pub fn is_domain_enabled(&self, stem: &str) -> bool {
        self.domains.iter().any(|d| d == stem)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_domains_is_sorted_and_unique() {
        let mut sorted = ALL_DOMAINS;
        sorted.sort_unstable();
        assert_eq!(ALL_DOMAINS, sorted);
        assert!(
            ALL_DOMAINS.windows(2).all(|pair| pair[0] != pair[1]),
            "名字重复了"
        );
    }

    #[test]
    fn default_opens_every_domain_on_android_only() {
        let default = DictionariesConfig::default();
        if cfg!(target_os = "android") {
            assert_eq!(default.domains.len(), ALL_DOMAINS.len());
        } else {
            assert_eq!(default.domains, DEFAULT_DOMAINS);
        }
    }

    #[test]
    fn enabled_checks_match_the_lists() {
        let config = DictionariesConfig::default();
        if cfg!(target_os = "android") {
            assert!(config.is_domain_enabled("medicine"));
        } else {
            assert!(config.is_domain_enabled("idioms"));
            assert!(!config.is_domain_enabled("medicine"));
        }
        // 用户词库在就加载，只有列进 disabled 的才跳过
        assert!(config.is_enabled("mine"));
    }
}
