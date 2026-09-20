package app.qingjian.android

import android.content.pm.PackageManager
import android.content.res.Configuration
import android.inputmethodservice.InputMethodService
import android.os.Build
import android.os.SystemClock
import android.util.Log
import android.view.KeyEvent
import android.view.View
import android.view.inputmethod.EditorInfo
import java.io.File
import java.io.IOException
import java.util.Locale

/**
 * 青简的输入法服务。
 *
 * 能打字、能选词、中 / 英可切——敲字母出候选，点候选（或空格）上屏到应用里。
 * 数字 / 符号面板、翻页手势的更多花样、无障碍都还没做，见 `docs/design/keyboard.md`。
 *
 * 这里是青简唯一碰安卓输入框的地方：Rust 那边只产「要上屏的文本」与「要原样转发的按键」，
 * 用什么 API 送出去是这一层的事。
 */
class QingjianImeService : InputMethodService() {
    /** Rust 侧的会话句柄，0 表示没打开。 */
    private var handle = 0L

    /** 输入视图。`onFinishInput` 里也要用它重画，所以记一份。 */
    private var inputView: QingjianSurfaceView? = null

    override fun onCreate() {
        super.onCreate()

        val dictionary = ensureBundled(DICTIONARY)
        if (dictionary == null) {
            Log.e(TAG, "词库解不出来，输入法用不了")
            return
        }

        handle = QingjianNative.open(
            dictionary.absolutePath,
            Locale.getDefault().toLanguageTag(),
            ensureExtras()?.absolutePath ?: "",
        )
        if (handle == 0L) {
            Log.e(TAG, "会话打开失败")
            return
        }
        Log.i(TAG, "会话已打开，词库 ${dictionary.length() / 1024} KB")
    }

    /**
     * 把随包的资源解到应用私有目录，返回文件；解不出来返回 null。
     *
     * 标记文件记的是**这个 APK 是什么时候装的**（`lastUpdateTime`），所以升级一次就自动重解一遍，
     * 不用维护版本号。字体 10 MB，不这么记的话每次启动都要白拷。
     */
    private fun ensureBundled(name: String, dir: File = filesDir): File? {
        val target = File(dir, name)
        val stamp = File(filesDir, ".$name.installed")
        val revision = installedAt()
        if (target.isFile && stamp.isFile && stamp.readText() == revision) {
            return target
        }
        return try {
            dir.mkdirs()
            assets.open(name).use { input ->
                target.outputStream().use { output -> input.copyTo(output) }
            }
            stamp.writeText(revision)
            target
        } catch (error: IOException) {
            Log.e(TAG, "随包资源 $name 解不出来", error)
            null
        }
    }

    /** emoji 字体与 emoji 表，解到一个子目录里一起交给 Rust。 */
    private fun ensureExtras(): File? {
        val dir = File(filesDir, BUNDLE_DIR)
        for (name in EMOJI_ASSETS) {
            if (ensureBundled(name, dir) == null) {
                return null
            }
        }
        return dir
    }

    /** 这个 APK 是什么时候装上的。换一次安装就变，用来决定随包资源要不要重解。 */
    private fun installedAt(): String {
        val info = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            packageManager.getPackageInfo(packageName, PackageManager.PackageInfoFlags.of(0))
        } else {
            @Suppress("DEPRECATION")
            packageManager.getPackageInfo(packageName, 0)
        }
        return info.lastUpdateTime.toString()
    }

    override fun onCreateInputView(): View {
        val view = QingjianSurfaceView(this)
        inputView = view
        popup = KeyPopup(this)
        // 直接按屏幕宽度配一次，不等视图量出来——视图的初始高度是 0，安卓不会给 0 高的视图
        // 发尺寸变化回调，等它就成了死锁。宽度变了（转屏）时再走 onConfigure。
        configure(view)
        view.onConfigure = { configure(view) }
        view.onTouch = { action, pointer, x, y ->
            val started = SystemClock.elapsedRealtime()
            val flags = QingjianNative.touch(handle, action, pointer, x, y)
            afterInput(view, flags, started)
        }
        // 长按连发：计时器在视图那边，到点问 Rust「这根手指按住的键要不要再来一下」。
        // 哪个键连发是输入语义，壳不判断——Rust 那边没按着该连发的键就什么也不做。
        view.onRepeat = { pointer ->
            val started = SystemClock.elapsedRealtime()
            val flags = QingjianNative.repeat(handle, pointer)
            afterInput(view, flags, started)
        }
        return view
    }

    /**
     * 一次输入（敲键、连发、长按）之后的收尾：上屏、镜像拼音、重画脏了的面。
     *
     * 打出慢帧：这一整套（引擎查询 + 画两张位图 + 过 JNI + 传成 Bitmap）都在触摸回调里
     * 同步做，一次超过一帧的时间打字就会跟不上手感。慢了就报出来。连发走的是同一条路，
     * 所以这里也是连发的耗时观测点——连发要是慢，手感一样钝。
     */
    private fun afterInput(view: QingjianSurfaceView, flags: Int, started: Long) {
        // 先上屏再镜像拼音：上屏会把组字区替换掉，剩下的拼音要紧跟着补回去
        if (flags and QingjianNative.FLAG_COMMIT != 0) {
            deliver()
        }
        if (flags and QingjianNative.FLAG_PREEDIT != 0) {
            mirrorPreedit()
        }
        if (flags and QingjianNative.FLAG_BAR != 0) {
            refreshBar(view)
        }
        if (flags and QingjianNative.FLAG_KEYBOARD != 0) {
            refreshKeyboard(view)
        }
        refreshPopup(view)
        val elapsed = SystemClock.elapsedRealtime() - started
        if (elapsed >= SLOW_TOUCH_MS) {
            Log.w(TAG, "这一下花了 ${elapsed}ms，打字会跟不上手感")
        }
    }

    /**
     * 按住键时那张预览气泡：贴上去、挪到 Rust 算好的位置；没在预览就收起来。
     *
     * 每次输入之后都问一次。没按住键时 Rust 那边立刻就回空（不做任何绘制），
     * 所以这一次调用很便宜——不必再加一个「气泡变了」的位掩码。
     */
    private fun refreshPopup(view: QingjianSurfaceView) {
        if (handle == 0L) return
        val bytes = QingjianNative.popupSurface(handle)
        if (bytes == null || bytes.isEmpty()) {
            popup?.dismiss()
            return
        }
        val origin = QingjianNative.popupOrigin(handle)
        if (origin == null || origin.size < 2) {
            popup?.dismiss()
            return
        }
        QingjianNative.toBitmap(bytes)?.let { bitmap ->
            popup?.show(view, bitmap, origin[0], origin[1])
        }
    }

    /**
     * 把引擎攒下的东西交给应用：先发原样按键，再上屏文本。
     *
     * 两者不会同时有——按键是「拼音已经空了，退格 / 回车该由应用自己处理」那一路。
     */
    private fun deliver() {
        val connection = currentInputConnection ?: return
        QingjianNative.takeCommands(handle)?.forEach { code ->
            val keyCode = when (code) {
                QingjianNative.COMMAND_BACKSPACE -> KeyEvent.KEYCODE_DEL
                QingjianNative.COMMAND_ENTER -> KeyEvent.KEYCODE_ENTER
                // 移光标只能用方向键：输入法不知道光标前后有什么，也没有「挪一格」的 API
                QingjianNative.COMMAND_MOVE_LEFT -> KeyEvent.KEYCODE_DPAD_LEFT
                QingjianNative.COMMAND_MOVE_RIGHT -> KeyEvent.KEYCODE_DPAD_RIGHT
                else -> return@forEach
            }
            connection.sendKeyEvent(KeyEvent(KeyEvent.ACTION_DOWN, keyCode))
            connection.sendKeyEvent(KeyEvent(KeyEvent.ACTION_UP, keyCode))
        }
        QingjianNative.takeCommit(handle)?.let { connection.commitText(it, 1) }
    }

    /** 把拼音镜像到输入框；空串表示没在组句，结束组字。 */
    private fun mirrorPreedit() {
        val connection = currentInputConnection ?: return
        val preedit = QingjianNative.takePreedit(handle)
        if (preedit.isEmpty()) {
            connection.finishComposingText()
        } else {
            connection.setComposingText(preedit, 1)
        }
    }

    /** 换应用时把没上屏的拼音丢掉，免得在 A 应用敲的拼音跑到 B 应用里。 */
    /**
     * 键盘每次弹出来都**回字母页**：收起来再弹出来不该还停在数字页。
     *
     * 挂这儿而不是 `onFinishInput`：BACK 收起键盘时安卓**不结束输入**（`onFinishInput` 不触发），
     * 只有这个回调一定到。**不动拼音**——那会儿用户只是把键盘收了。
     */
    override fun onStartInputView(info: EditorInfo?, restarting: Boolean) {
        super.onStartInputView(info, restarting)
        if (handle == 0L) return
        val flags = QingjianNative.resetPanel(handle)
        inputView?.let { view ->
            if (flags and QingjianNative.FLAG_BAR != 0) refreshBar(view)
            if (flags and QingjianNative.FLAG_KEYBOARD != 0) refreshKeyboard(view)
        }
    }

    override fun onFinishInput() {
        super.onFinishInput()
        if (handle == 0L) return
        // 清空顺带把键盘复位回字母页，掩码里会带 FLAG_KEYBOARD——照同一套收尾重画一遍，
        // 不然键盘收起来再弹出来还停着上一页的键
        val flags = QingjianNative.clear(handle)
        popup?.dismiss()
        inputView?.let { view ->
            if (flags and QingjianNative.FLAG_BAR != 0) refreshBar(view)
            if (flags and QingjianNative.FLAG_KEYBOARD != 0) refreshKeyboard(view)
        }
        mirrorPreedit()
    }

    /** 按当前屏幕宽度告诉 Rust 该画多宽，并把整块输入视图的高度要回来。 */
    /** 键预览气泡的浮动小窗。位图由 Rust 画，这里只贴上去。 */
    private var popup: KeyPopup? = null

    private fun configure(view: QingjianSurfaceView) {
        if (handle == 0L) return
        val metrics = resources.displayMetrics
        QingjianNative.configure(
            handle,
            metrics.widthPixels / metrics.density,
            metrics.density,
            view.bottomInsetPoints,
            isDark(),
        )
        // 高度不用自己算：视图按两张位图加起来的像素高自己量
        refreshBar(view)
        refreshKeyboard(view)
    }

    /**
     * 重新取一张候选条位图贴上。Rust 那边没脏就会返回同一张，不会白画。
     *
     * **空字节串表示「这一条现在不该在」**（没在组句，整条收起来了），要把视图上那张撤掉——
     * 光不更新是不够的，旧位图还占着高度、键盘会停在被顶上去的位置。
     */
    private fun refreshBar(view: QingjianSurfaceView) {
        if (handle == 0L) return
        val bytes = QingjianNative.barSurface(handle)
        if (bytes == null || bytes.isEmpty()) {
            view.setBar(null)
            return
        }
        QingjianNative.toBitmap(bytes)?.let(view::setBar)
    }

    /** 重新取一张键盘位图贴上。Rust 那边没脏就会返回同一张，不会白画。 */
    private fun refreshKeyboard(view: QingjianSurfaceView) {
        if (handle == 0L) return
        val bytes = QingjianNative.keyboardSurface(handle)
        if (bytes == null || bytes.isEmpty()) {
            Log.e(TAG, "键盘没画出来（渲染器没建起来，或者还没配过宽度）")
            return
        }
        QingjianNative.toBitmap(bytes)?.let(view::setKeyboard)
    }

    /** 系统现在是深色吗。 */
    private fun isDark(): Boolean =
        (resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK) ==
            Configuration.UI_MODE_NIGHT_YES

    override fun onDestroy() {
        if (handle != 0L) {
            QingjianNative.close(handle)
            handle = 0L
        }
        super.onDestroy()
    }

    private companion object {
        const val TAG = "Qingjian"

        /** 随包词库的文件名，放在应用私有目录。 */
        const val DICTIONARY = "dict.qj"

        /** emoji 字体与 emoji 表（在 APK 的 assets 里，启动时解到私有目录的 [BUNDLE_DIR]）。 */
        val EMOJI_ASSETS = listOf("NotoColorEmoji.ttf", "emoji-zh.tsv", "emoji-en.tsv")

        /** emoji 那几个文件解到私有目录时用的子目录名。 */
        const val BUNDLE_DIR = "emoji"

        /** 一次触摸超过这么多毫秒就报一声（约一帧）；打字手感的分水岭。 */
        const val SLOW_TOUCH_MS = 16L
    }
}
