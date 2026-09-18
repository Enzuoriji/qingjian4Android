package app.qingjian.android

import android.graphics.Bitmap
import java.nio.ByteBuffer

/**
 * Rust 侧的入口。
 *
 * 函数名与 `apps/android/src/bridge.rs` 里的 JNI 符号一一对应，改一边必须同时改另一边；
 * 类名与包名参与符号名（`Java_app_qingjian_android_QingjianNative_<方法>`），同样不能单独动。
 */
object QingjianNative {
    /** 打开会话，返回句柄；失败返回 0。`locale` 决定中日同形字取哪家字形。 */
    external fun open(dictionaryPath: String, locale: String): Long

    /** 释放会话。 */
    external fun close(handle: Long)

    /** 敲入一个字符。 */
    external fun push(handle: Long, ch: Char)

    /** 清空缓冲区。 */
    external fun clear(handle: Long)

    /** 当前候选的文本，一行一个。调试阶段用来验证链路，正式版换成自绘位图。 */
    external fun candidates(handle: Long): String

    /**
     * 位图通路的探针（M0 临时件）：`which` 0 是色块、其余是文字。
     *
     * 返回 8 字节头（宽、高各一个 Int）+ 预乘 RGBA 像素，见 [toBitmap]。
     */
    external fun probe(handle: Long, which: Int): ByteArray?

    /** 探针用（M0 临时件）：报告探针文字落到哪些字族，` | ` 分隔。 */
    external fun probeTrace(handle: Long): String

    /**
     * 告诉 Rust 键盘有多宽（点）、屏幕密度、底部被系统占掉多高、是不是深色。
     * **返回键盘总共该有多高（点，含底部那一段）**。
     */
    external fun configureKeyboard(
        handle: Long,
        width: Float,
        density: Float,
        bottomInset: Float,
        dark: Boolean,
    ): Float

    /** 键盘的位图（8 字节头 + 预乘 RGBA）。没配过宽度时是空数组。 */
    external fun keyboardSurface(handle: Long): ByteArray?

    /** 一次触摸，返回 [FLAG_BAR] 那样的位掩码，告诉壳哪些面要重取。 */
    external fun touch(handle: Long, action: Int, x: Float, y: Float): Int

    /** 最近一次按下又抬起的键的调试名称（M1 临时件）。 */
    external fun lastTouched(handle: Long): String

    /** 候选条变了。 */
    const val FLAG_BAR = 1

    /** 键盘变了。 */
    const val FLAG_KEYBOARD = 2

    /** 有文本要上屏。 */
    const val FLAG_COMMIT = 4

    /** 拼音行变了。 */
    const val FLAG_PREEDIT = 8

    init {
        System.loadLibrary("qingjian_android")
    }

    /**
     * 把 [probe] 回来的字节串还原成 Bitmap。
     *
     * 安卓 `ARGB_8888` 的内存布局就是预乘 RGBA，`copyPixelsFromBuffer` 是裸内存拷贝、不做转换，
     * 所以 tiny-skia 的像素可以原样直通。**不要用 `setPixels(int[])`**：那条路径假定的是非预乘数据。
     */
    fun toBitmap(bytes: ByteArray): Bitmap? {
        if (bytes.size < 8) return null
        val buffer = ByteBuffer.wrap(bytes)
        val width = buffer.int
        val height = buffer.int
        if (width <= 0 || height <= 0 || bytes.size < 8 + width * height * 4) return null
        val bitmap = Bitmap.createBitmap(width, height, Bitmap.Config.ARGB_8888)
        // 位图密度设成 NONE，免得绘制时被按屏幕密度缩放
        bitmap.density = Bitmap.DENSITY_NONE
        bitmap.copyPixelsFromBuffer(buffer)
        return bitmap
    }
}
