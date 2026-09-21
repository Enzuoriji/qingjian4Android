//! 「最近用过的一串东西」的落盘：剪贴板历史、表情面板的「最近」都是它。
//!
//! 两处的形状一模一样——一串用户用过的文本，去重、插最前、有个条数上限、一改就写盘。
//! 差别只有上限多少与文件名，所以是**一个类型两处用**（泛化之前它叫 `Clipboard`）。

use std::path::{Path, PathBuf};

use qingjian_core::storage::{read_text_lossy, write_atomic_str};

use crate::error::LearningError;

/// 剪贴板历史最多记几条——超了丢最旧的。
///
/// 参考项目 fcitx5-android 是可配的（缺省 20，界面里能改），搜狗存 500 条。
/// 我们存 50：够翻十来屏，也不至于让那个文件长得没法看。
pub const CLIPBOARD_LIMIT: usize = 50;

/// 表情面板的「最近」记几条。一排格子 15 个，两排够用。
pub const EMOJI_RECENT_LIMIT: usize = 30;

/// 一串「最近用过的东西」的落盘：**一行一条、最新的在最前**。
///
/// 与这个 crate 里别的表有两点不一样：
///
/// - **一改就写盘**（[`Self::remember`] / [`Self::remove`] / [`Self::clear`] 里直接写）。
///   别的表可以攒着等 `flush`——那些是统计与偏好，丢一两条无所谓；剪贴板丢的是
///   「用户刚复制的那一条」，而输入法进程在安卓上随时会被杀，攒着写就等于白记。
/// - **内容是任意文本**（可能带换行、制表符），所以每条要转义（见 [`escape`]），
///   不然一行一条这个格式会被内容里的换行撑破。
#[derive(Debug)]
pub struct Recent {
    /// 历史，最新的在最前。
    entries: Vec<String>,

    /// 写回的路径；`None` 只在内存里记（壳没给数据目录时）。
    path: Option<PathBuf>,

    /// 最多记几条——超了丢最旧的。
    limit: usize,
}

impl Default for Recent {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            path: None,
            // 只在内存里记时也得有个上限：`truncate(0)` 会把刚记的立刻丢掉。
            // 真用的时候一律走 `open`，上限由调用方按用途给。
            limit: CLIPBOARD_LIMIT,
        }
    }
}

impl Recent {
    /// 从文件加载（文件不在就从零开始，头一次 [`Self::remember`] 时建）；
    /// 读不了就退回只在内存里记——这点数据丢了不该让输入法起不来。
    pub fn open(path: impl Into<PathBuf>, limit: usize) -> Self {
        let path = path.into();
        match read_text_lossy(&path) {
            Ok(text) => Self {
                entries: text.as_deref().map(|t| parse(t, limit)).unwrap_or_default(),
                path: Some(path),
                limit,
            },
            Err(error) => {
                tracing::warn!(path = %path.display(), %error, "最近用过的那串读不了，这一次只在内存里记");
                Self {
                    limit,
                    ..Self::default()
                }
            }
        }
    }

    /// 历史，最新的在最前。
    pub fn entries(&self) -> &[String] {
        &self.entries
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// 记一条：**同一条再复制只挪到最前**，不重复记；超了 [`LIMIT`] 丢最旧的。空白不收。
    ///
    /// 返回真的变了没有——没变（就是最前面那条）就不必重画、也不必写盘。
    pub fn remember(&mut self, text: &str) -> bool {
        if text.trim().is_empty() {
            return false;
        }
        if self.entries.first().is_some_and(|first| first == text) {
            return false;
        }
        self.entries.retain(|entry| entry != text);
        self.entries.insert(0, text.to_owned());
        self.entries.truncate(self.limit);
        self.save();
        true
    }

    /// 删掉第 `index` 条（**整份里的下标**）。
    pub fn remove(&mut self, index: usize) -> bool {
        if index >= self.entries.len() {
            return false;
        }
        self.entries.remove(index);
        self.save();
        true
    }

    /// 清空整份历史。
    pub fn clear(&mut self) -> bool {
        if self.entries.is_empty() {
            return false;
        }
        self.entries.clear();
        self.save();
        true
    }

    /// 把不想要的条目去掉（谁说了算由调用方给）。变了就写盘。
    ///
    /// 用在「上次记下的东西这回画不出来了」这种时候：字体换了、或者渲染器加载的清单变了。
    pub fn retain(&mut self, keep: impl Fn(&str) -> bool) {
        let before = self.entries.len();
        self.entries.retain(|entry| keep(entry));
        if self.entries.len() != before {
            self.save();
        }
    }

    /// 写回文件。**失败只记日志**：内存里那份还在，这一次会话照常用。
    fn save(&self) {
        let Some(path) = self.path.as_deref() else {
            return;
        };
        if let Err(error) = self.save_to(path) {
            tracing::warn!(path = %path.display(), %error, "剪贴板历史保存失败");
        }
    }

    fn save_to(&self, path: &Path) -> Result<(), LearningError> {
        let mut text = String::from("# 剪贴板历史，一行一条、最新的在最前\n");
        for entry in &self.entries {
            text.push_str(&escape(entry));
            text.push('\n');
        }
        write_atomic_str(path, &text)?;
        Ok(())
    }
}

/// 一条文本写成一行：反斜杠、换行、回车、制表符都转义掉。
///
/// 反过来是 [`unescape`]。别的表直接按 `\t` 切就行——那些字段是词和数字，不会带这些字符；
/// 剪贴板里什么都可能有。
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out
}

/// [`escape`] 的反操作。认不出的转义原样留着（宁可多两个字符，也别把内容吃掉）。
fn unescape(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('t') => out.push('\t'),
            Some('\\') => out.push('\\'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            // 行尾孤零零一个反斜杠：原样留着
            None => out.push('\\'),
        }
    }
    out
}

/// 按行解析。坏行（转义完是空的）跳过，超 [`LIMIT`] 的丢掉——文件被手改过也不至于出事。
fn parse(text: &str, limit: usize) -> Vec<String> {
    text.lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(unescape)
        .filter(|entry| !entry.trim().is_empty())
        .take(limit)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{CLIPBOARD_LIMIT, Recent, escape, unescape};

    fn dir(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("qingjian-clipboard-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn escapes_the_characters_that_would_break_the_format() {
        let rough = "第一行\n第二行\t带制表符\\还有反斜杠\r回车";
        let flat = escape(rough);
        assert!(!flat.contains('\n'), "转义完不该还有真换行：{flat}");
        assert_eq!(unescape(&flat), rough, "转一圈该原样回来");
        // 认不出的转义原样留着，别把内容吃掉
        assert_eq!(unescape("a\\qb"), "a\\qb");
    }

    #[test]
    fn remembers_the_newest_first_and_does_not_repeat() {
        let mut clipboard = Recent::default();
        clipboard.remember("一");
        clipboard.remember("二");
        assert_eq!(clipboard.entries(), ["二", "一"], "最新的在最前");

        // 同一条再复制只是挪到最前，不重复记
        assert!(clipboard.remember("一"));
        assert_eq!(clipboard.entries(), ["一", "二"]);
        assert!(!clipboard.remember("一"), "已经在最前面了，什么也没变");
        assert!(!clipboard.remember("   "), "空白不收");
    }

    #[test]
    fn drops_the_oldest_past_the_limit() {
        let mut clipboard = Recent::default();
        for index in 0..CLIPBOARD_LIMIT + 10 {
            clipboard.remember(&format!("第 {index} 条"));
        }
        assert_eq!(clipboard.len(), CLIPBOARD_LIMIT);
        assert_eq!(
            clipboard.entries()[0],
            format!("第 {} 条", CLIPBOARD_LIMIT + 9)
        );
        assert!(
            !clipboard.entries().contains(&"第 0 条".to_owned()),
            "最旧的该被丢掉了"
        );
    }

    #[test]
    fn round_trips_through_the_file() {
        let dir = dir("round");
        let path = dir.join("clipboard.tsv");
        let mut clipboard = Recent::open(&path, CLIPBOARD_LIMIT);
        assert!(clipboard.is_empty(), "文件还没有，从头开始");

        clipboard.remember("带\n换行的");
        clipboard.remember("普通一条");
        assert!(path.is_file(), "记一条就该落盘");

        let reloaded = Recent::open(&path, CLIPBOARD_LIMIT);
        assert_eq!(
            reloaded.entries(),
            ["普通一条", "带\n换行的"],
            "换个会话读回来该一模一样"
        );

        // 删一条、清空，也都是立刻落盘
        let mut reopened = Recent::open(&path, CLIPBOARD_LIMIT);
        assert!(reopened.remove(0));
        assert!(!reopened.remove(9), "下标越界什么也不做");
        assert_eq!(
            Recent::open(&path, CLIPBOARD_LIMIT).entries(),
            ["带\n换行的"]
        );
        assert!(reopened.clear());
        assert!(Recent::open(&path, CLIPBOARD_LIMIT).is_empty());

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_hand_edited_file_does_not_blow_up() {
        let dir = dir("broken");
        let path = dir.join("clipboard.tsv");
        std::fs::write(&path, "# 头\n\n好的\n   \n也好的\n").unwrap();
        let clipboard = Recent::open(&path, CLIPBOARD_LIMIT);
        assert_eq!(clipboard.entries(), ["好的", "也好的"], "空行与注释跳过");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
