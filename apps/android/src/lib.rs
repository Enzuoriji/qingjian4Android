//! 青简安卓壳：把平台无关的 Core 接到 Android 的 InputMethodService。
//!
//! Kotlin 侧只经过 [`bridge`] 里那几个函数跟这里打交道。候选窗由 `qingjian-render`
//! 出位图、Kotlin 只贴图；这里不碰任何窗口。

mod action;
mod bridge;
mod error;
mod keyboard;
mod session;
mod surface;
mod touch;

pub use error::SessionError;
pub use session::Session;
