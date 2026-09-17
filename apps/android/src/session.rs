//! 一次输入法会话：持有 [`Engine`]，把 Kotlin 侧的调用翻译成它的方法。

use std::path::Path;

use qingjian_core::Engine;
use qingjian_dictionary::Dictionary;

use crate::error::SessionError;

/// 安卓壳持有的会话状态。
///
/// Android 一个输入法进程只服务当前前台应用，所以这里跟 macOS 一样是进程级单例，
/// 不像 Windows 要按会话分派。
pub struct Session {
    /// 输入引擎。
    engine: Engine,
}

impl Session {
    /// 打开词库并建好引擎。
    pub fn open(dictionary_path: &Path) -> Result<Self, SessionError> {
        let dictionary = Dictionary::from_path(dictionary_path)?;

        Ok(Self {
            engine: Engine::new(dictionary),
        })
    }

    /// 敲入一个字符。
    pub fn push(&mut self, c: char) {
        self.engine.push(c);
    }

    /// 清空缓冲区。
    pub fn clear(&mut self) {
        self.engine.clear();
    }

    /// 当前候选的文本，调试阶段用来验证链路。
    pub fn candidates(&self) -> Vec<String> {
        match self.engine.query() {
            Ok(query) => query
                .candidates
                .items
                .iter()
                .map(|c| c.text.clone())
                .collect(),
            Err(_) => Vec::new(),
        }
    }
}
