//! 日志：把 `tracing` 接到 Android 的 logcat 上。
//!
//! 在这之前**一个订阅器都没装**——`tracing` 只是个门面，没人收就什么都不发。于是
//! `qingjian-core` / `qingjian-render` 里那些 `info!` / `warn!` / `error!` 全被丢掉了，
//! 真出问题时 `adb logcat` 里只剩 Kotlin 那几句，Rust 这半边是瞎的。
//!
//! 标签与 Kotlin 侧 `QingjianImeService.TAG` 一致，所以 `adb logcat -s Qingjian` 两边都收得到。
//! 宿主机（跑测试）上不装：输出交给 `cargo test` 自己的捕获。

#[cfg(target_os = "android")]
use std::ffi::CString;
#[cfg(target_os = "android")]
use std::io::{self, Write};

#[cfg(target_os = "android")]
use android_log_sys::{__android_log_write, LogPriority};
#[cfg(target_os = "android")]
use tracing::{Level, Metadata};
#[cfg(target_os = "android")]
use tracing_subscriber::filter::LevelFilter;
#[cfg(target_os = "android")]
use tracing_subscriber::fmt::MakeWriter;

/// logcat 的标签，与 Kotlin 侧一致。
#[cfg(target_os = "android")]
const TAG: &str = "Qingjian";

/// 单条消息的长度上限。再长会被 logcat 截掉，不如我们自己分段发，至少字都在。
#[cfg(target_os = "android")]
const MAX_CHUNK: usize = 4000;

/// 装订阅器，装过了就什么都不做。
///
/// 用 `try_init` 而不是 `init`：一个进程只该有一个订阅器，重复调用不该把输入法进程打崩。
///
/// 级别写死，不走 `RUST_LOG`——安卓上没地方设环境变量，要看更细的日志得改成 `DEBUG` 重编。
/// 调试包缺省就开到 `DEBUG`。
#[cfg(target_os = "android")]
pub fn init() {
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;

    let level = if cfg!(debug_assertions) {
        LevelFilter::DEBUG
    } else {
        LevelFilter::INFO
    };

    let _ = tracing_subscriber::registry()
        .with(level)
        .with(
            tracing_subscriber::fmt::layer()
                // logcat 自己带时间戳，再打一遍是重复
                .without_time()
                .with_ansi(false)
                .with_writer(Logcat),
        )
        .try_init();
}

/// 宿主机上没有 logcat，什么都不装。
#[cfg(not(target_os = "android"))]
pub fn init() {}

/// 往 logcat 写的口子。
#[cfg(target_os = "android")]
struct Logcat;

#[cfg(target_os = "android")]
impl<'a> MakeWriter<'a> for Logcat {
    type Writer = LogcatWriter;

    fn make_writer(&'a self) -> Self::Writer {
        LogcatWriter::new(LogPriority::INFO)
    }

    /// 级别映射成 logcat 的优先级——`error!` 得能在 `adb logcat Qingjian:E` 里筛出来。
    fn make_writer_for(&'a self, meta: &Metadata<'_>) -> Self::Writer {
        LogcatWriter::new(match *meta.level() {
            Level::ERROR => LogPriority::ERROR,
            Level::WARN => LogPriority::WARN,
            Level::INFO => LogPriority::INFO,
            Level::DEBUG => LogPriority::DEBUG,
            Level::TRACE => LogPriority::VERBOSE,
        })
    }
}

/// 一条事件的 writer。
///
/// **要攒到换行再发**：fmt 那一层一条事件分好几次 `write`（前缀、正文、字段、换行各一次），
/// 来一次发一条的话，一条日志会被拆成好几行 logcat。
#[cfg(target_os = "android")]
struct LogcatWriter {
    priority: LogPriority,

    line: String,
}

#[cfg(target_os = "android")]
impl LogcatWriter {
    fn new(priority: LogPriority) -> Self {
        Self {
            priority,
            line: String::new(),
        }
    }
}

#[cfg(target_os = "android")]
impl Write for LogcatWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.line.push_str(&String::from_utf8_lossy(buf));
        while let Some(end) = self.line.find('\n') {
            let rest = self.line.split_off(end + 1);
            let mut line = std::mem::replace(&mut self.line, rest);
            line.pop();
            emit(self.priority, &line);
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if !self.line.is_empty() {
            let line = std::mem::take(&mut self.line);
            emit(self.priority, &line);
        }
        Ok(())
    }
}

/// writer 用完就丢，收尾那下 `flush` 得在这儿补上——最后一行可能没有换行。
#[cfg(target_os = "android")]
impl Drop for LogcatWriter {
    fn drop(&mut self) {
        let _ = self.flush();
    }
}

/// 发一条给 logcat。
#[cfg(target_os = "android")]
fn emit(priority: LogPriority, line: &str) {
    let Ok(tag) = CString::new(TAG) else {
        return;
    };
    // C 字符串到 NUL 就断，日志里真出了 NUL 先剔掉，免得后半截凭空消失
    let line = line.replace('\0', "");
    for chunk in line.as_bytes().chunks(MAX_CHUNK) {
        let Ok(text) = CString::new(chunk) else {
            continue;
        };
        // SAFETY: 两个指针都指向本函数里活着的 C 字符串，logcat 只读它们。
        unsafe {
            __android_log_write(priority as i32, tag.as_ptr(), text.as_ptr());
        }
    }
}
