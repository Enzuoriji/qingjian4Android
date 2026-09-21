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
    /**
     * 打开会话，返回句柄；失败返回 0。
     *
     * `locale` 决定中日同形字取哪家字形；`emojiFontPath` 是随包 emoji 字体的路径，
     * 空串表示用系统里那张（系统那张安卓 15 起画不出来，见 `assets/emoji/README.md`）。
     */
    external fun open(dictionaryPath: String, locale: String, emojiFontPath: String): Long

    /** 释放会话。 */
    external fun close(handle: Long)

    /**
     * 清空缓冲区。换应用时调，免得在 A 应用敲的拼音跑到 B 应用里。
     *
     * 返回与 [touch] 同一种位掩码：它顺带把键盘复位回字母页，**那时键盘位图得重画**，
     * 壳照这个掩码走同一套收尾。
     */
    external fun clear(handle: Long): Int

    /**
     * 告诉 Rust 输入视图有多宽（点）、**屏幕在当前方向上的高度**（点）、屏幕密度、
     * 底部被系统占掉多高、是不是深色。
     * **返回整块输入视图总共该有多高（点）**：候选条 + 键盘 + 底部让开的那一段。
     *
     * `screenHeight` 是给键盘定高度的（屏幕大的手机键盘也大），竖屏取长边、横屏取短边
     * ——怎么算见 `QingjianImeService.screenHeightPoints`。
     */
    external fun configure(
        handle: Long,
        width: Float,
        screenHeight: Float,
        density: Float,
        bottomInset: Float,
        dark: Boolean,
        landscape: Boolean,
    ): Float

    /** 候选条的位图（8 字节头 + 预乘 RGBA）。没配过宽度时是空数组。 */
    external fun barSurface(handle: Long): ByteArray?

    /** 键盘的位图（8 字节头 + 预乘 RGBA）。没配过宽度时是空数组。 */
    external fun keyboardSurface(handle: Long): ByteArray?

    /**
     * 按住键时那张预览气泡的位图。**空数组表示「这个浮动小窗现在不该在」**。
     *
     * 与 [barSurface] 一个规矩：拿到空要把窗收起来，不是「不更新」。
     */
    external fun popupSurface(handle: Long): ByteArray?

    /**
     * 气泡位图左上角该摆在哪儿（**整块输入视图**的像素）：`[x, y]`。
     *
     * 空数组表示没在预览。摆哪儿由 Rust 算好——壳只把窗挪过去，不掺和布局。
     */
    external fun popupOrigin(handle: Long): FloatArray?

    /** 键盘又要弹出来了：把页复位回字母页，返回与 [touch] 同一种位掩码。 */
    external fun resetPanel(handle: Long): Int

    /**
     * 一次触摸，返回 [FLAG_BAR] 那样的位掩码，告诉壳哪些面要重取。
     *
     * `pointer` 是安卓给的 pointer id，`x` / `y` 是**那根手指**的坐标，且坐标是整块输入视图的
     * （候选条在上、键盘在下，Rust 那边按 y 分派）。多点触控要按根分开算，别传 `event.x`。
     */
    external fun touch(handle: Long, action: Int, pointer: Int, x: Float, y: Float): Int

    /**
     * 长按连发：计时器到点了，问一次「这根手指按住的那个键要不要再来一下」。
     *
     * 返回与 [touch] 同一种位掩码。计时器在壳这边（安卓有现成的 `Handler`），
     * **该不该触发由 Rust 判**——哪个键连发是输入语义，壳不需要知道。
     */
    external fun repeat(handle: Long, pointer: Int): Int

    /**
     * 空格上移光标的**一拍**：壳的心跳到点了，问「这一拍走几格」。
     *
     * 走几格由 Rust 按**手指离开按下那点多远**算（越远越快），壳不必知道死区与速度。
     */
    external fun cursorTick(handle: Long, pointer: Int): Int

    /**
     * 系统剪贴板里新复制了东西：记一条进历史（剪贴板页画的就是它）。
     *
     * **敏感内容与空白在壳这边就滤掉了**（见 `QingjianImeService.readClipboard`），不会送到这儿来。
     * 返回与 [touch] 同一种位掩码：剪贴板页开着时那份列表要重画。
     */
    external fun clipboardChanged(handle: Long, text: String): Int

    /**
     * 一根手指抬起了，报上它的速度（**像素/秒**，横向向右为正、纵向向下为正，
     * 就是 `VelocityTracker` 的单位与方向）。
     *
     * 壳只管量（安卓自带 `VelocityTracker`，自己算得再去摸时间戳），
     * **该不该接着滑、滑多远由 Rust 定**——只有刚才真的滚过的那根手指才算数。
     * 横竖两个分量各归各的：候选条横着滑、剪贴板列表竖着滑。
     */
    external fun fling(handle: Long, pointer: Int, velocityX: Float, velocityY: Float): Int

    /**
     * 惯性滑行的**一拍**：壳的帧到点了，问「过去 `dtMs` 毫秒，这一拍该挪多少」。
     *
     * 返回的掩码里还有 [FLAG_FLING] 就接着敲下一帧，没有就停。
     * 帧的节拍在壳、衰减曲线在 Rust——与 [repeat]、[cursorTick] 同一个分工。
     */
    external fun flingStep(handle: Long, dtMs: Float): Int

    /**
     * 该镜像给应用的拼音行（取走并清掉脏标记）。
     *
     * 空串表示没在组句，壳应当 `finishComposingText()`。
     */
    external fun takePreedit(handle: Long): String

    /** 取走要上屏的文本（并清掉）；这次没有返回 null。 */
    external fun takeCommit(handle: Long): String?

    /** 取走要原样交给应用的按键编号（并清掉）；这次没有返回空数组。编号见 [COMMAND_BACKSPACE]。 */
    external fun takeCommands(handle: Long): IntArray?

    /** [takeCommands] 里的编号：删应用里的一个字符。 */
    const val COMMAND_BACKSPACE = 1

    /** [takeCommands] 里的编号：回车。 */
    const val COMMAND_ENTER = 2

    /** [takeCommands] 里的编号：光标左移一格。 */
    const val COMMAND_MOVE_LEFT = 3

    /** [takeCommands] 里的编号：光标右移一格。 */
    const val COMMAND_MOVE_RIGHT = 4

    /** [takeCommands] 里的编号：把光标前面整段清掉。 */
    const val COMMAND_CLEAR_ALL = 5

    /** 候选条变了。 */
    const val FLAG_BAR = 1

    /** 键盘变了。 */
    const val FLAG_KEYBOARD = 2

    /** 有文本要上屏。 */
    const val FLAG_COMMIT = 4

    /** 拼音行变了。 */
    const val FLAG_PREEDIT = 8

    /** 候选条还在惯性滑行：壳接着排下一帧（问 [flingStep]）。 */
    const val FLAG_FLING = 16

    init {
        System.loadLibrary("qingjian_android")
    }

    /**
     * 把 [barSurface] / [keyboardSurface] 回来的字节串还原成 Bitmap。
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
