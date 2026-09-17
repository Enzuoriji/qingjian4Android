//! JNI 入口：Kotlin 侧 `app.qingjian.android.QingjianNative` 调用的那几个函数。
//!
//! 句柄是 `Box<Session>` 的裸指针：Kotlin 侧持有它，用完交回来释放。
//! 这里刻意只暴露文本进出的最小接口——候选窗是自绘位图，Kotlin 不需要看到
//! `Candidate` 对象，跨语言传的永远只是字符串与数字。

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;

use jni::JNIEnv;
use jni::objects::{JObject, JString};
use jni::sys::{jchar, jlong, jstring};

use crate::session::Session;

/// 把句柄还原成会话。
///
/// # Safety
///
/// 句柄必须来自 [`Java_app_qingjian_android_QingjianNative_open`] 且尚未释放。
unsafe fn from_handle<'a>(handle: jlong) -> Option<&'a mut Session> {
    if handle == 0 {
        return None;
    }

    Some(unsafe { &mut *(handle as *mut Session) })
}

/// 打开会话并返回句柄；失败返回 0。
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_qingjian_android_QingjianNative_open(
    mut env: JNIEnv,
    _this: JObject,
    dictionary_path: JString,
) -> jlong {
    let Ok(path) = env.get_string(&dictionary_path) else {
        return 0;
    };
    let path = PathBuf::from(String::from(path));

    // panic 穿出 JNI 边界会直接把进程带走，这里拦一次
    match catch_unwind(AssertUnwindSafe(|| Session::open(&path))) {
        Ok(Ok(session)) => Box::into_raw(Box::new(session)) as jlong,
        _ => 0,
    }
}

/// 释放会话。
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_qingjian_android_QingjianNative_close(
    _env: JNIEnv,
    _this: JObject,
    handle: jlong,
) {
    if handle == 0 {
        return;
    }

    let _ = catch_unwind(AssertUnwindSafe(|| {
        drop(unsafe { Box::from_raw(handle as *mut Session) });
    }));
}

/// 敲入一个字符。`ch` 是 UTF-16 码元，BMP 之内与 `char` 同值。
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_qingjian_android_QingjianNative_push(
    _env: JNIEnv,
    _this: JObject,
    handle: jlong,
    ch: jchar,
) {
    let Some(session) = (unsafe { from_handle(handle) }) else {
        return;
    };

    if let Some(c) = char::from_u32(u32::from(ch)) {
        session.push(c);
    }
}

/// 清空缓冲区。
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_qingjian_android_QingjianNative_clear(
    _env: JNIEnv,
    _this: JObject,
    handle: jlong,
) {
    if let Some(session) = unsafe { from_handle(handle) } {
        session.clear();
    }
}

/// 当前候选的文本，一行一个。调试阶段用来验证链路，正式版换成自绘位图。
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_qingjian_android_QingjianNative_candidates(
    env: JNIEnv,
    _this: JObject,
    handle: jlong,
) -> jstring {
    let text = unsafe { from_handle(handle) }
        .map(|session| session.candidates().join("\n"))
        .unwrap_or_default();

    match env.new_string(text) {
        Ok(value) => value.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}
