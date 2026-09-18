//! JNI 入口：Kotlin 侧 `app.qingjian.android.QingjianNative` 调用的那几个函数。
//!
//! 句柄是 `Box<Session>` 的裸指针：Kotlin 侧持有它，用完交回来释放。
//! 跨语言只传字节与数字——候选条与键盘都由 `qingjian-render` 出位图（见 [`crate::surface`]），
//! Kotlin 不需要看到 `Candidate` 之类的对象。

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;

use jni::JNIEnv;
use jni::objects::{JObject, JString};
use jni::sys::{jboolean, jbyteArray, jchar, jfloat, jint, jlong, jstring};

use crate::session::Session;
use crate::touch::MotionAction;

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

/// 打开会话并返回句柄；失败返回 0。`locale` 决定中日同形字取哪家字形。
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_qingjian_android_QingjianNative_open(
    mut env: JNIEnv,
    _this: JObject,
    dictionary_path: JString,
    locale: JString,
) -> jlong {
    let Ok(path) = env.get_string(&dictionary_path) else {
        return 0;
    };
    let path = PathBuf::from(String::from(path));
    let locale = env
        .get_string(&locale)
        .map(String::from)
        .unwrap_or_else(|_| "zh-CN".to_owned());

    // panic 穿出 JNI 边界会直接把进程带走，这里拦一次
    match catch_unwind(AssertUnwindSafe(|| Session::open(&path, &locale))) {
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

/// 位图通路的探针（M0 临时件，键盘接上之后删）：`which` 0 是色块、其余是文字。
///
/// 返回 8 字节头（宽高，各 u32 大端）+ 预乘 RGBA，见 [`crate::surface`]。
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_qingjian_android_QingjianNative_probe(
    env: JNIEnv,
    _this: JObject,
    handle: jlong,
    which: jint,
) -> jbyteArray {
    let bytes = match unsafe { from_handle(handle) } {
        Some(session) => {
            catch_unwind(AssertUnwindSafe(|| session.probe(which))).unwrap_or_default()
        }
        None => Vec::new(),
    };

    match env.byte_array_from_slice(&bytes) {
        Ok(array) => array.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// 探针用（M0 临时件）：报告探针文字落到了哪些字族，` | ` 分隔。
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_qingjian_android_QingjianNative_probeTrace(
    env: JNIEnv,
    _this: JObject,
    handle: jlong,
) -> jstring {
    let families = match unsafe { from_handle(handle) } {
        Some(session) => catch_unwind(AssertUnwindSafe(|| session.trace())).unwrap_or_default(),
        None => Vec::new(),
    };

    match env.new_string(families.join(" | ")) {
        Ok(value) => value.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// 壳报告输入视图的宽度（点）、屏幕密度、底部被系统占掉的高度、明暗，
/// 返回整块输入视图**总共该有多高**（点）：候选条 + 键盘 + 底部让开的那一段。
///
/// 高度要回传：安卓按视图量出来的尺寸给输入法窗口大小，壳不知道高度就会把窗口撑满整屏。
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_qingjian_android_QingjianNative_configure(
    _env: JNIEnv,
    _this: JObject,
    handle: jlong,
    width: jfloat,
    density: jfloat,
    bottom_inset: jfloat,
    dark: jboolean,
) -> jfloat {
    match unsafe { from_handle(handle) } {
        Some(session) => catch_unwind(AssertUnwindSafe(|| {
            session.configure(width, density, bottom_inset, dark != 0)
        }))
        .unwrap_or(0.0),
        None => 0.0,
    }
}

/// 候选条的位图（8 字节头 + 预乘 RGBA）。没配过宽度或渲染器不可用时是空数组。
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_qingjian_android_QingjianNative_barSurface(
    env: JNIEnv,
    _this: JObject,
    handle: jlong,
) -> jbyteArray {
    let bytes = match unsafe { from_handle(handle) } {
        Some(session) => {
            catch_unwind(AssertUnwindSafe(|| session.bar_surface())).unwrap_or_default()
        }
        None => Vec::new(),
    };

    match env.byte_array_from_slice(&bytes) {
        Ok(array) => array.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// 键盘的位图（8 字节头 + 预乘 RGBA）。没配过宽度或渲染器不可用时是空数组。
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_qingjian_android_QingjianNative_keyboardSurface(
    env: JNIEnv,
    _this: JObject,
    handle: jlong,
) -> jbyteArray {
    let bytes = match unsafe { from_handle(handle) } {
        Some(session) => {
            catch_unwind(AssertUnwindSafe(|| session.keyboard_surface())).unwrap_or_default()
        }
        None => Vec::new(),
    };

    match env.byte_array_from_slice(&bytes) {
        Ok(array) => array.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// 一次触摸，返回 [`crate::session::flags`] 的位掩码。
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_qingjian_android_QingjianNative_touch(
    _env: JNIEnv,
    _this: JObject,
    handle: jlong,
    action: jint,
    x: jfloat,
    y: jfloat,
) -> jint {
    match unsafe { from_handle(handle) } {
        Some(session) => catch_unwind(AssertUnwindSafe(|| {
            session.touch(MotionAction::from_motion(action), x, y)
        }))
        .unwrap_or(0),
        None => 0,
    }
}

/// 最近一次按下又抬起的键的调试名称（M1 临时件，接上引擎后删）。
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_qingjian_android_QingjianNative_lastTouched(
    env: JNIEnv,
    _this: JObject,
    handle: jlong,
) -> jstring {
    let name = match unsafe { from_handle(handle) } {
        Some(session) => {
            catch_unwind(AssertUnwindSafe(|| session.last_touched_name())).unwrap_or_default()
        }
        None => String::new(),
    };

    match env.new_string(name) {
        Ok(value) => value.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}
