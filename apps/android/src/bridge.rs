//! JNI 入口：Kotlin 侧 `app.qingjian.android.QingjianNative` 调用的那几个函数。
//!
//! 句柄是 `Box<Session>` 的裸指针：Kotlin 侧持有它，用完交回来释放。
//! 跨语言只传字节与数字——候选条与键盘都由 `qingjian-render` 出位图（见 [`crate::surface`]），
//! Kotlin 不需要看到 `Candidate` 之类的对象。

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};

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
/// （emoji 字体与 emoji 表），空串表示没有——那时用系统 emoji 字体、也不出 emoji 候选；
/// `data_dir` 是**可写**的数据目录（安卓的 `filesDir`），剪贴板历史这类要留住的东西落在那儿，
/// 空串表示没有（那就只在内存里记）。
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_qingjian_android_QingjianNative_open(
    mut env: JNIEnv,
    _this: JObject,
    dictionary_path: JString,
    locale: JString,
    bundle_dir: JString,
    data_dir: JString,
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
    let data_dir = env
        .get_string(&data_dir)
        .map(String::from)
        .unwrap_or_default();
    let data_dir = (!data_dir.is_empty()).then(|| PathBuf::from(data_dir));

    // panic 穿出 JNI 边界会直接把进程带走，这里拦一次
    match catch_unwind(AssertUnwindSafe(|| {
        Session::open(&path, &locale, bundle_dir.as_deref(), data_dir.as_deref())
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

/// 把学习数据落盘。
///
/// 壳在几个时机调它：键盘窗口藏起来（`onWindowHidden`，主路径）、焦点离开输入框（`onFinishInput`）、
/// 进程退出前、以及键盘开着时每 60 秒兜一次。
/// **进程退出那次必须在 [`Java_app_qingjian_android_QingjianNative_close`] 之前**——
/// 句柄是 `Box<Session>` 的裸指针，`close` 就是 `drop`，会话没有 `Drop`、不会自己落盘。
///
/// 返回 0 = 正常，1 = 内部 panic 被拦下（壳目前不看这个值，留着是为了别把 panic 咽得无声无息）。
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_qingjian_android_QingjianNative_flushLearning(
    _env: JNIEnv,
    _this: JObject,
    handle: jlong,
) -> jint {
    match unsafe { from_handle(handle) } {
        Some(session) => match catch_unwind(AssertUnwindSafe(|| session.flush_learning())) {
            Ok(()) => 0,
            Err(_) => 1,
        },
        None => 0,
    }
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

/// 壳报告输入视图的宽度（点）、**屏幕在当前方向上的高度**（点）、屏幕密度、
/// 底部被系统占掉的高度、明暗，
/// 返回整块输入视图**总共该有多高**（点）：候选条 + 键盘 + 底部让开的那一段。
///
/// 高度要回传：安卓按视图量出来的尺寸给输入法窗口大小，壳不知道高度就会把窗口撑满整屏。
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_qingjian_android_QingjianNative_configure(
    _env: JNIEnv,
    _this: JObject,
    handle: jlong,
    width: jfloat,
    screen_height: jfloat,
    density: jfloat,
    bottom_inset: jfloat,
    dark: jboolean,
    landscape: jboolean,
) -> jfloat {
    match unsafe { from_handle(handle) } {
        Some(session) => catch_unwind(AssertUnwindSafe(|| {
            session.configure(
                width,
                screen_height,
                density,
                bottom_inset,
                dark != 0,
                landscape != 0,
            )
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

/// 返回键：把开着的那层收掉（展开面板 / 工具页这些）。
///
/// **返回 0 表示这一下不归输入法管**，壳照常把返回交给应用（[`Session::dismiss`] 的约定）：
/// 反过来「非 0 = 我处理了」也成立。
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_qingjian_android_QingjianNative_dismiss(
    _env: JNIEnv,
    _this: JObject,
    handle: jlong,
) -> jint {
    match unsafe { from_handle(handle) } {
        Some(session) => catch_unwind(AssertUnwindSafe(|| session.dismiss())).unwrap_or(0),
        None => 0,
    }
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
        .unwrap_or_else(|panic| {
            // panic 了：这一下**不兑现**，但得留句话。静默吞掉的话，真机上表现就是
            // 「偶尔掉个字母」——连从哪儿查都不知道（这是 2026-09-23 补的）。
            tracing::error!(
                action,
                pointer,
                reason = panic_message(&panic),
                "touch 里 panic 了"
            );
            0
        }),
        None => 0,
    }
}

/// 从 `catch_unwind` 捞到的那个 panic 里把消息取出来。
///
/// `panic!` 的载荷可能是 `&str` 也可能是 `String`（`panic!("{x}")` 那种），两种都认；
/// 都取不到就给一句占位，别让日志里空着。
fn panic_message(panic: &(dyn std::any::Any + Send)) -> &str {
    panic
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| panic.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("(panic 没有带消息)")
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

/// 系统剪贴板里新复制了东西：记一条进历史（剪贴板页画的就是它）。
///
/// 敏感内容与空白**壳那边就滤掉了**，不会走到这儿来（见 `QingjianImeService.readClipboard`）——
/// 读得到什么、该不该读是平台的事；记几条、怎么去重是这边的事。
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_qingjian_android_QingjianNative_clipboardChanged(
    mut env: JNIEnv,
    _this: JObject,
    handle: jlong,
    text: JString,
) -> jint {
    let Ok(text) = env.get_string(&text) else {
        return 0;
    };
    let text = String::from(text);
    match unsafe { from_handle(handle) } {
        Some(session) => {
            catch_unwind(AssertUnwindSafe(|| session.note_clipboard(&text))).unwrap_or(0)
        }
        None => 0,
    }
}

/// 一根手指抬起了，报上它的速度（**像素/秒**，横向向右为正、纵向向下为正，
/// 都是 `VelocityTracker` 的单位与方向）。
///
/// 够快就让刚才滚的那个接着滑一段——候选条横着滑、剪贴板列表竖着滑，两个分量各归各的。
/// 甩不甩、甩多远由 Rust 定（[`crate::session::Session::start_fling`]），
/// 壳只管量速度（安卓自带 `VelocityTracker`，自己算得再去摸时间戳）。
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_qingjian_android_QingjianNative_fling(
    _env: JNIEnv,
    _this: JObject,
    handle: jlong,
    pointer: jint,
    velocity_x: jfloat,
    velocity_y: jfloat,
) -> jint {
    match unsafe { from_handle(handle) } {
        Some(session) => catch_unwind(AssertUnwindSafe(|| {
            session.start_fling(pointer, velocity_x, velocity_y)
        }))
        .unwrap_or(0),
        None => 0,
    }
}

/// 惯性的**一拍**：壳的帧到点了，问「过去 `dt` 毫秒，这一拍该挪多少」。
///
/// 返回的掩码里有 [`crate::session::flags::FLING`] 就接着敲下一帧，没有就停。
/// 帧的节拍在壳、手感（衰减曲线）在 Rust——与长按连发、移光标同一个分工。
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_qingjian_android_QingjianNative_flingStep(
    _env: JNIEnv,
    _this: JObject,
    handle: jlong,
    dt: jfloat,
) -> jint {
    match unsafe { from_handle(handle) } {
        Some(session) => catch_unwind(AssertUnwindSafe(|| session.fling_step(dt))).unwrap_or(0),
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

/// 取走「用户点了设置页」（位掩码里带 `SETTINGS` 时调用）。壳据此开设置页。
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_qingjian_android_QingjianNative_takeSettings(
    _env: JNIEnv,
    _this: JObject,
    handle: jlong,
) -> jboolean {
    match unsafe { from_handle(handle) } {
        Some(session) => {
            catch_unwind(AssertUnwindSafe(|| session.take_settings())).unwrap_or(false) as jboolean
        }
        None => 0,
    }
}

/// 报上光标前后的文本（云联想拿它当上下文）。壳在键盘弹出来时问一次应用的输入框。
///
/// 太长不要紧，引擎按 `[predict] lookback / lookahead` 自己裁。
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_qingjian_android_QingjianNative_setSurrounding(
    mut env: JNIEnv,
    _this: JObject,
    handle: jlong,
    before: JString,
    after: JString,
) {
    let (Some(before), Some(after)) = (string_arg(&mut env, &before), string_arg(&mut env, &after))
    else {
        return;
    };
    if let Some(session) = unsafe { from_handle(handle) } {
        let _ = catch_unwind(AssertUnwindSafe(|| session.set_surrounding(before, after)));
    }
}

/// 私密输入框（密码框）里：不学、不记、**不发云端**。壳按 `EditorInfo.inputType` 判。
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_qingjian_android_QingjianNative_setPrivate(
    _env: JNIEnv,
    _this: JObject,
    handle: jlong,
    private: jboolean,
) {
    if let Some(session) = unsafe { from_handle(handle) } {
        let _ = catch_unwind(AssertUnwindSafe(|| session.set_private(private != 0)));
    }
}

/// 云联想有结果回来了没有。壳在掩码带 `PREDICTING` 时按拍子问，返回同一种位掩码。
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_qingjian_android_QingjianNative_pollPrediction(
    _env: JNIEnv,
    _this: JObject,
    handle: jlong,
) -> jint {
    match unsafe { from_handle(handle) } {
        Some(session) => catch_unwind(AssertUnwindSafe(|| session.poll_prediction())).unwrap_or(0),
        None => 0,
    }
}

/// 配置文件变了没有；变了就重读并应用，返回位掩码（没变是 0）。
///
/// 壳**只在键盘弹出来时调**（`onStartInputView`）：用户从设置页回来时键盘必然重弹一次，
/// 这一条就够；桌面那种每秒轮询在安卓是白养一个定时器。见 [`Session::poll_config`]。
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_qingjian_android_QingjianNative_configPoll(
    _env: JNIEnv,
    _this: JObject,
    handle: jlong,
) -> jint {
    match unsafe { from_handle(handle) } {
        Some(session) => catch_unwind(AssertUnwindSafe(|| session.poll_config())).unwrap_or(0),
        None => 0,
    }
}

/// 从 Kotlin 的字符串取一个 Rust `String`。
fn string_arg(env: &mut JNIEnv, value: &JString) -> Option<String> {
    env.get_string(value).ok().map(String::from)
}

/// 把一个字符串交给 Kotlin。空串是有意义的值（配置那条路上表示「写成功了」）。
fn into_jstring(env: &JNIEnv, text: &str) -> jstring {
    match env.new_string(text) {
        Ok(value) => value.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

// 下面这几个是**给设置页用的**，都只吃数据目录、**不吃会话句柄**：
// 设置页与输入法服务是两个组件，会话可能在设置页开着的时候就被销毁了，
// 手里那个 handle 就是野指针。设置页只跟 `config.toml` 打交道最安全。
// 读写都走 [`crate::settings`]，与会话读的是同一份文件。

/// 读整份配置，返回 JSON 信封（`{"ok":true,"config":{…}}` 或 `{"ok":false,"error":"…"}`）。
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_qingjian_android_QingjianNative_configRead(
    mut env: JNIEnv,
    _this: JObject,
    data_dir: JString,
) -> jstring {
    let Some(dir) = string_arg(&mut env, &data_dir) else {
        return std::ptr::null_mut();
    };
    let text = catch_unwind(AssertUnwindSafe(|| {
        crate::settings::read_json(Path::new(&dir))
    }))
    .unwrap_or_else(|_| r#"{"ok":false,"error":"读取失败"}"#.to_owned());
    into_jstring(&env, &text)
}

/// 写一个开关。返回**空串表示成功**，非空是错误文案。
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_qingjian_android_QingjianNative_configSetBool(
    mut env: JNIEnv,
    _this: JObject,
    data_dir: JString,
    section: JString,
    key: JString,
    value: jboolean,
) -> jstring {
    let (Some(dir), Some(section), Some(key)) = (
        string_arg(&mut env, &data_dir),
        string_arg(&mut env, &section),
        string_arg(&mut env, &key),
    ) else {
        return std::ptr::null_mut();
    };
    let error = catch_unwind(AssertUnwindSafe(|| {
        crate::settings::set_bool(Path::new(&dir), &section, &key, value != 0)
    }))
    .unwrap_or_else(|_| "写入失败".to_owned());
    into_jstring(&env, &error)
}

/// 写一个整数（震动时长这类）。返回空串表示成功。
///
/// 收 `jint` 而不是 `jlong`：Kotlin 侧写 `Int` 更顺，而且跨语言**类型必须对上**
/// （`Int` 的 JNI 签名是 `I`，与 `jlong` 对不上会直接崩）。
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_qingjian_android_QingjianNative_configSetInt(
    mut env: JNIEnv,
    _this: JObject,
    data_dir: JString,
    section: JString,
    key: JString,
    value: jint,
) -> jstring {
    let (Some(dir), Some(section), Some(key)) = (
        string_arg(&mut env, &data_dir),
        string_arg(&mut env, &section),
        string_arg(&mut env, &key),
    ) else {
        return std::ptr::null_mut();
    };
    let error = catch_unwind(AssertUnwindSafe(|| {
        crate::settings::set_int(Path::new(&dir), &section, &key, i64::from(value))
    }))
    .unwrap_or_else(|_| "写入失败".to_owned());
    into_jstring(&env, &error)
}

/// 写一个字符串（学习语言、震动风格这类枚举也走它）。返回空串表示成功。
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_qingjian_android_QingjianNative_configSetString(
    mut env: JNIEnv,
    _this: JObject,
    data_dir: JString,
    section: JString,
    key: JString,
    value: JString,
) -> jstring {
    let (Some(dir), Some(section), Some(key), Some(value)) = (
        string_arg(&mut env, &data_dir),
        string_arg(&mut env, &section),
        string_arg(&mut env, &key),
        string_arg(&mut env, &value),
    ) else {
        return std::ptr::null_mut();
    };
    let error = catch_unwind(AssertUnwindSafe(|| {
        crate::settings::set_string(Path::new(&dir), &section, &key, &value)
    }))
    .unwrap_or_else(|_| "写入失败".to_owned());
    into_jstring(&env, &error)
}

/// 写一串字符串（领域词库那种清单）。`values` 是**一段 JSON 数组文本**，
/// 传数组要在这边逐个取元素、还得处理元素取不出来的情况，为一个键不值得。
/// 返回空串表示成功。
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_qingjian_android_QingjianNative_configSetArray(
    mut env: JNIEnv,
    _this: JObject,
    data_dir: JString,
    section: JString,
    key: JString,
    values: JString,
) -> jstring {
    let (Some(dir), Some(section), Some(key), Some(raw)) = (
        string_arg(&mut env, &data_dir),
        string_arg(&mut env, &section),
        string_arg(&mut env, &key),
        string_arg(&mut env, &values),
    ) else {
        return std::ptr::null_mut();
    };
    // 解析不了就**别写**：当作空数组写进去会把用户的词库全关掉，那不是他要的
    let Ok(values) = serde_json::from_str::<Vec<String>>(&raw) else {
        return into_jstring(&env, "清单格式不对，没有写入");
    };
    let error = catch_unwind(AssertUnwindSafe(|| {
        crate::settings::set_array(Path::new(&dir), &section, &key, &values)
    }))
    .unwrap_or_else(|_| "写入失败".to_owned());
    into_jstring(&env, &error)
}

/// 开始测试云服务连接：**空串表示开始了**，非空是没能开始的原因（比如没填密钥）。
///
/// 与上面那些配置读写一样**不吃会话句柄**——设置页跟会话是两回事。
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_qingjian_android_QingjianNative_cloudTestStart(
    mut env: JNIEnv,
    _this: JObject,
    data_dir: JString,
) -> jstring {
    let Some(dir) = string_arg(&mut env, &data_dir) else {
        return std::ptr::null_mut();
    };
    let error = catch_unwind(AssertUnwindSafe(|| {
        crate::settings::cloud_test_start(Path::new(&dir))
    }))
    .unwrap_or_else(|_| "测试没能开始".to_owned());
    into_jstring(&env, &error)
}

/// 取测试连接的结果，JSON：`{"done":false}` 还没回来，`{"done":true,"ok":…,"text":…}` 是结果。
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_qingjian_android_QingjianNative_cloudTestPoll(
    env: JNIEnv,
    _this: JObject,
) -> jstring {
    let text = catch_unwind(AssertUnwindSafe(crate::settings::cloud_test_poll))
        .unwrap_or_else(|_| r#"{"done":false}"#.to_owned());
    into_jstring(&env, &text)
}

/// 列出随包的领域词库，返回 JSON 数组（每项 `{"stem":…,"name":…}`，名字是词库文件里那个中文名）。
///
/// 设置页照它列勾选框——**壳那边不硬编码那 11 本**，以后加一本词库这边跟着就有了。
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_qingjian_android_QingjianNative_domainList(
    mut env: JNIEnv,
    _this: JObject,
    bundle_dir: JString,
) -> jstring {
    let Some(dir) = string_arg(&mut env, &bundle_dir) else {
        return std::ptr::null_mut();
    };
    let text = catch_unwind(AssertUnwindSafe(|| {
        crate::settings::domain_list(Path::new(&dir))
    }))
    .unwrap_or_else(|_| "[]".to_owned());
    into_jstring(&env, &text)
}
