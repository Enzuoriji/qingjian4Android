package app.qingjian.android

/**
 * Rust 侧的入口。
 *
 * 函数名与 `apps/android/src/bridge.rs` 里的 JNI 符号一一对应，改一边必须同时改另一边；
 * 类名与包名参与符号名（`Java_app_qingjian_android_QingjianNative_<方法>`），同样不能单独动。
 */
object QingjianNative {
    /** 打开会话，返回句柄；失败返回 0。 */
    external fun open(dictionaryPath: String): Long

    /** 释放会话。 */
    external fun close(handle: Long)

    /** 敲入一个字符。 */
    external fun push(handle: Long, ch: Char)

    /** 清空缓冲区。 */
    external fun clear(handle: Long)

    /** 当前候选的文本，一行一个。调试阶段用来验证链路，正式版换成自绘位图。 */
    external fun candidates(handle: Long): String

    init {
        System.loadLibrary("qingjian_android")
    }
}
