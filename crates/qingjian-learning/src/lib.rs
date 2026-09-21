//! 用户词频学习与输入日志的落盘。
//!
//! [`FrequencyLearner`] 存聚合数据（选择次数、输入串选择、用户词、个人英文词、个人 n-gram，都是 TSV）；
//! [`InputLog`] 逐条记上屏（jsonl），给离线回归评测与个人模型用；[`UsageStats`] 按天数打了多少字（`usage.tsv`），
//! [`VocabularyBook`] 记学习语言的译词看过 / 上屏过几次（`user-vocab.tsv`，候选里标生词的依据）；
//! [`Recent`] 是「最近用过的一串东西」（剪贴板历史、表情面板的「最近」都用它）——
//! 它不是「学习数据」，但同样是用户侧落盘、
//! 同样要「坏了不能让输入法起不来」，所以住这儿。

mod error;
mod frequency_learner;
mod input_log;
mod recent;
mod usage_stats;
mod vocabulary_book;

pub use error::LearningError;
pub use frequency_learner::FrequencyLearner;
pub use input_log::InputLog;
pub use recent::{CLIPBOARD_LIMIT, EMOJI_RECENT_LIMIT, Recent};
pub use usage_stats::UsageStats;
pub use vocabulary_book::VocabularyBook;
