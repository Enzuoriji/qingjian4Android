//! Android 的字体清单与回退表。
//!
//! 文件在 `/system/fonts/`：先按常见文件名挑，再扫一遍目录补上 OEM 改过名字或位置的（去重，清单里的排在前面）。
//! 只加载界面字体、中日韩、emoji 几个文件，思路与 `linux.rs` 一致。
//!
//! 回退表是这份文件存在的另一半理由：cosmic-text 在安卓上落进 `fallback/other.rs`（**空表**），
//! 而 `NotoSansCJK-Regular.ttc` 是集合、含 SC / TC / JP / KR / HK 多个面，不指定回退会让中文
//! 落到第一个面（日文字形）。macOS 与 Windows 靠 cosmic-text 自带的平台表避开了这个坑，安卓没有，
//! 所以这里补一份，内容照抄它的 `fallback/unix.rs`。

use std::path::PathBuf;

use cosmic_text::Fallback;
use unicode_script::Script;

/// 安卓放系统字体的地方。
const FONT_DIR: &str = "/system/fonts";

/// 界面字体：系统的无衬线正文。
pub(super) fn ui_fonts() -> Vec<PathBuf> {
    files(
        &[
            "Roboto-Regular.ttf",
            "RobotoStatic-Regular.ttf",
            "NotoSans-Regular.ttf",
        ],
        &["roboto"],
    )
}

/// 中日韩字体。收的是 `.ttc` 集合，里面的面由回退表按 locale 挑。
pub(super) fn script_fonts(_locale: &str) -> Vec<PathBuf> {
    files(
        &[
            "NotoSansCJK-Regular.ttc",
            "NotoSansSC-Regular.otf",
            "NotoSansTC-Regular.otf",
            "DroidSansFallbackFull.ttf",
        ],
        &[
            "cjk",
            "notosanssc",
            "notosanstc",
            "notosansjp",
            "notosanskr",
            "fallback",
        ],
    )
}

/// 彩色 emoji。
pub(super) fn emoji_fonts() -> Vec<PathBuf> {
    files(
        &["NotoColorEmoji.ttf", "NotoColorEmojiLegacy.ttf"],
        &["emoji"],
    )
}

/// 清单里的路径在前，扫目录扫到的在后；两边都没有就返回空（调用方会跳过加载）。
fn files(names: &[&str], keywords: &[&str]) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = names
        .iter()
        .map(|name| PathBuf::from(FONT_DIR).join(name))
        .collect();
    for path in scanned(keywords) {
        if !out.contains(&path) {
            out.push(path);
        }
    }
    out
}

/// 扫一遍字体目录，挑文件名里含任一关键词的。列不动目录（个别系统会拦）就当没有，不报错。
fn scanned(keywords: &[&str]) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(FONT_DIR) else {
        tracing::debug!(dir = FONT_DIR, "字体目录列不出来，只用固定清单");
        return Vec::new();
    };
    let mut out: Vec<PathBuf> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            let hit = name.ends_with(".ttf") || name.ends_with(".ttc") || name.ends_with(".otf");
            (hit && keywords.iter().any(|word| name.contains(word))).then(|| entry.path())
        })
        .collect();
    out.sort();
    out
}

/// 安卓的字体回退表。cosmic-text 在该平台上给的是空表，见文件头。
pub(super) struct AndroidFallback;

impl Fallback for AndroidFallback {
    fn common_fallback(&self) -> &[&'static str] {
        &["Roboto", "Noto Sans", "Noto Color Emoji"]
    }

    fn forbidden_fallback(&self) -> &[&'static str] {
        &[]
    }

    fn script_fallback(&self, script: Script, locale: &str) -> &[&'static str] {
        match script {
            Script::Han | Script::Bopomofo => han_unification(locale),
            Script::Hiragana | Script::Katakana => han_unification("ja"),
            Script::Hangul => han_unification("ko"),
            _ => &[],
        }
    }
}

/// 中日同形字按 locale 选 `NotoSansCJK-Regular.ttc` 里的哪个面。
///
/// locale 的写法不止一种（`zh-CN` / `zh-Hans-CN` / `zh-Hant-TW`），所以按前缀与变体标记认，不写死整串。
fn han_unification(locale: &str) -> &'static [&'static str] {
    if locale.starts_with("ja") {
        &["Noto Sans CJK JP"]
    } else if locale.starts_with("ko") {
        &["Noto Sans CJK KR"]
    } else if locale.contains("Hant") || locale.contains("TW") || locale.contains("HK") {
        &["Noto Sans CJK TC", "Noto Sans CJK HK"]
    } else {
        &["Noto Sans CJK SC"]
    }
}
