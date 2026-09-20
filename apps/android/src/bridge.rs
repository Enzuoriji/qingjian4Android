//! JNI 入口：Kotlin 侧 `app.qingjian.android.QingjianNative` 调用的那几个函数。
//!
//! 句柄是 `Box<Session>` 的裸指针：Kotlin 侧持有它，用完交回来释放。
//! 跨语言只传字节与数字——候选条与键盘都由 `qingjian-render` 出位图（见 [`crate::surface`]），
//! Kotlin 不需要看到 `Candidate` 之类的对象。

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;

use jni::JNIEnv;
use jni::objects::{JObject, JString};
use jni::sys::{jboolean, jbyteArray, jfloat, jfloatArray, jint, jintArray, jlong, jstring};

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

/// 打开会话并返回句柄；失败返回 0。
///
/// `locale` 决定中日同形字取哪家字形；`bundle_dir` 是壳从 APK 里解出来的随包资源目录
/// （emoji 字体与 emoji 表），空串表示没有——那时用系统 emoji 字体、也不出 emoji 候选。
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_qingjian_android_QingjianNative_open(
    mut env: JNIEnv,
    _this: JObject,
    dictionary_path: JString,
    locale: JString,
    bundle_dir: JString,
) -> jlong {
    // 装日志。放这儿是图它一定早于任何会打日志的调用——**建会话失败的时候最需要日志**，
    // 装晚了那几条错误信息就正好错过了。装第二次是空操作。
    crate::logging::init();

    let Ok(path) = env.get_string(&dictionary_path) else {
        return 0;
    };
    let path = PathBuf::from(String::from(path));
    let locale = env
        .get_string(&locale)
        .map(String::from)
        .unwrap_or_else(|_| "zh-CN".to_owned());
    let bundle_dir = env
        .get_string(&bundle_dir)
        .map(String::from)
        .unwrap_or_default();
    let bundle_dir = (!bundle_dir.is_empty()).then(|| PathBuf::from(bundle_dir));

    // panic 穿出 JNI 边界会直接把进程带走，这里拦一次
    match catch_unwind(AssertUnwindSafe(|| {
        Session::open(&path, &locale, bundle_dir.as_deref())
    })) {
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

/// 清空缓冲区。
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_qingjian_android_QingjianNative_clear(
    _env: JNIEnv,
    _this: JObject,
    handle: jlong,
) -> jint {
    match unsafe { from_handle(handle) } {
        Some(session) => catch_unwind(AssertUnwindSafe(|| session.clear())).unwrap_or(0),
        None => 0,
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

/// 按住键时那张预览气泡的位图（8 字节头 + 预乘 RGBA）。没在预览时是**空数组**。
///
/// 空表示「这个小窗现在不该在」，壳收到要把浮动小窗收起来——与候选条一个规矩。
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_qingjian_android_QingjianNative_popupSurface(
    env: JNIEnv,
    _this: JObject,
    handle: jlong,
) -> jbyteArray {
    let bytes = match unsafe { from_handle(handle) } {
        Some(session) => {
            catch_unwind(AssertUnwindSafe(|| session.popup_surface())).unwrap_or_default()
        }
        None => Vec::new(),
    };

    match env.byte_array_from_slice(&bytes) {
        Ok(array) => array.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// 气泡位图左上角该摆在哪儿（整块输入视图的像素，与触摸坐标同一套）：`[x, y]`。
///
/// 没在预览时是**空数组**。摆哪儿由 Rust 算好——壳只把浮动小窗挪到
/// 「视图在屏幕上的位置 + 这个偏移」，不掺和布局。
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_qingjian_android_QingjianNative_popupOrigin(
    env: JNIEnv,
    _this: JObject,
    handle: jlong,
) -> jfloatArray {
    let origin = match unsafe { from_handle(handle) } {
        Some(session) => catch_unwind(AssertUnwindSafe(|| session.popup_origin())).unwrap_or(None),
        None => None,
    };
    let Some((x, y)) = origin else {
        return env
            .new_float_array(0)
            .map_or(std::ptr::null_mut(), |array| array.into_raw());
    };

    let Ok(array) = env.new_float_array(2) else {
        return std::ptr::null_mut();
    };
    if env.set_float_array_region(&array, 0, &[x, y]).is_err() {
        return std::ptr::null_mut();
    }
    array.into_raw()
}

/// 键盘又要弹出来了：把页复位回字母页，返回 [`crate::session::flags`] 的位掩码。
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_qingjian_android_QingjianNative_resetPanel(
    _env: JNIEnv,
    _this: JObject,
    handle: jlong,
) -> jint {
    match unsafe { from_handle(handle) } {
        Some(session) => catch_unwind(AssertUnwindSafe(|| session.reset_panel())).unwrap_or(0),
        None => 0,
    }
}

/// 一次触摸，返回 [`crate::session::flags`] 的位掩码。
///
/// `pointer` 是安卓给的 pointer id，`x` / `y` 是**那根手指**的坐标——多点触控要按根分开算，
/// 传 `event.x`（永远是第 0 根）会让两根手指互相吃掉对方。
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_qingjian_android_QingjianNative_touch(
    _env: JNIEnv,
    _this: JObject,
    handle: jlong,
    action: jint,
    pointer: jint,
    x: jfloat,
    y: jfloat,
) -> jint {
    match unsafe { from_handle(handle) } {
        Some(session) => catch_unwind(AssertUnwindSafe(|| {
            session.touch(MotionAction::from_motion(action), pointer, x, y)
        }))
        .unwrap_or(0),
        None => 0,
    }
}

/// 长按连发：壳的计时器到点了，问一次「按住的那个键要不要再来一下」。
///
/// 计时器在壳那边（安卓有现成的 `Handler`），这里只回答该不该触发——
/// 哪个键连发是输入语义，放在 [`crate::action::repeats`]。
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_qingjian_android_QingjianNative_repeat(
    _env: JNIEnv,
    _this: JObject,
    handle: jlong,
    pointer: jint,
) -> jint {
    match unsafe { from_handle(handle) } {
        Some(session) => catch_unwind(AssertUnwindSafe(|| session.repeat(pointer))).unwrap_or(0),
        None => 0,
    }
}

/// 空格上移光标的**一拍**：壳的心跳到点了，问「这一拍走几格」。
///
/// 走几格由 Rust 按**手指离开按下那点多远**算（越远越快），壳不必知道死区与速度。
/// 计时在壳（有现成的 `Handler`）、节奏在 Rust——与长按连发同一个分工。
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_qingjian_android_QingjianNative_cursorTick(
    _env: JNIEnv,
    _this: JObject,
    handle: jlong,
    pointer: jint,
) -> jint {
    match unsafe { from_handle(handle) } {
        Some(session) => {
            catch_unwind(AssertUnwindSafe(|| session.cursor_tick(pointer))).unwrap_or(0)
        }
        None => 0,
    }
}

/// 该镜像给应用的拼音行（取走并清掉脏标记）。空串表示没在组句，壳应当 `finishComposingText`。
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_qingjian_android_QingjianNative_takePreedit(
    env: JNIEnv,
    _this: JObject,
    handle: jlong,
) -> jstring {
    let text = match unsafe { from_handle(handle) } {
        Some(session) => {
            catch_unwind(AssertUnwindSafe(|| session.take_preedit())).unwrap_or_default()
        }
        None => String::new(),
    };

    match env.new_string(text) {
        Ok(value) => value.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// 取走要上屏的文本（并清掉）；这次没有就返回 null。
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_qingjian_android_QingjianNative_takeCommit(
    env: JNIEnv,
    _this: JObject,
    handle: jlong,
) -> jstring {
    let text = match unsafe { from_handle(handle) } {
        Some(session) => {
            catch_unwind(AssertUnwindSafe(|| session.take_commit())).unwrap_or_default()
        }
        None => None,
    };
    let Some(text) = text else {
        return std::ptr::null_mut();
    };

    match env.new_string(text) {
        Ok(value) => value.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// 取走要原样交给应用的按键编号（并清掉）；这次没有就返回空数组。编号见 [`crate::action::Command::code`]。
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_qingjian_android_QingjianNative_takeCommands(
    env: JNIEnv,
    _this: JObject,
    handle: jlong,
) -> jintArray {
    let codes = match unsafe { from_handle(handle) } {
        Some(session) => {
            catch_unwind(AssertUnwindSafe(|| session.take_commands())).unwrap_or_default()
        }
        None => Vec::new(),
    };

    let Ok(array) = env.new_int_array(codes.len() as i32) else {
        return std::ptr::null_mut();
    };
    if !codes.is_empty() && env.set_int_array_region(&array, 0, &codes).is_err() {
        return std::ptr::null_mut();
    }
    array.into_raw()
}
