//! 安卓壳的错误类型。

use thiserror::Error;

/// 会话装配过程中的失败。
#[derive(Debug, Error)]
pub enum SessionError {
    /// 词库打不开：文件不存在、格式不对或 mmap 失败。
    #[error("failed to open dictionary: {0}")]
    Dictionary(#[from] qingjian_dictionary::DictionaryError),
}
