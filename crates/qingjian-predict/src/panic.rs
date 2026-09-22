//! 从 `catch_unwind` 的 payload 里捞 panic 的那句话。

/// 捞出来。捞不到就给个「未知原因」，别给空串。
///
/// **为什么非得主动捞**：安卓上 Rust 的 panic 走 stderr，而 stderr 没接到 logcat——
/// 线程崩了，消息凭空消失，日志里只剩「线程没了」这个结果，一点线索都没有
/// （2026-09-22 真机上「测试连接」一直转圈，最后显示「请求线程意外退出」，
///  是这句话把原因带回来的）。
pub(crate) fn message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(text) = payload.downcast_ref::<&str>() {
        (*text).to_owned()
    } else if let Some(text) = payload.downcast_ref::<String>() {
        text.clone()
    } else {
        "未知原因".to_owned()
    }
}
