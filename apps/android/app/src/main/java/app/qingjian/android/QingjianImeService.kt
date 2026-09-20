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
import android.view.inputmethod.ExtractedTextRequest
import android.view.inputmethod.InputConnection
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
        // 移光标：空格键上按着横滑时，壳按节拍问一次「这一拍走几格」。
        // 走几格、多快，全由 Rust 按位移量算——壳不必知道死区与速度曲线。
        view.onCursorTick = { pointer ->
            val started = SystemClock.elapsedRealtime()
            val flags = QingjianNative.cursorTick(handle, pointer)
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
        // 移光标可能一次来好几格，攒成**净位移**最后只调一次——每格都去问一遍
        // 光标在哪儿要多花几十次跨进程往返
        var cursor = 0
        var select = 0
        QingjianNative.takeCommands(handle)?.forEach { code ->
            when (code) {
                QingjianNative.COMMAND_MOVE_LEFT -> cursor -= 1
                QingjianNative.COMMAND_MOVE_RIGHT -> cursor += 1
                // 方向就在名字里：往左扩。所以是**减**，不是加
                QingjianNative.COMMAND_SELECT_LEFT -> select -= 1
                else -> {
                    // 别的命令一来（松手那条退格），这一个选字手势就算完了
                    selectAnchor = -1
                    selectSteps = 0
                    val keyCode = when (code) {
                        QingjianNative.COMMAND_BACKSPACE -> KeyEvent.KEYCODE_DEL
                        QingjianNative.COMMAND_ENTER -> KeyEvent.KEYCODE_ENTER
                        else -> return@forEach
                    }
                    connection.sendKeyEvent(KeyEvent(KeyEvent.ACTION_DOWN, keyCode))
                    connection.sendKeyEvent(KeyEvent(KeyEvent.ACTION_UP, keyCode))
                }
            }
        }
        if (cursor != 0) moveCursor(connection, cursor)
        if (select != 0) selectChars(connection, select)
        QingjianNative.takeCommit(handle)?.let { connection.commitText(it, 1) }
    }

    /** 退格上滑选字：手势开始时的光标位置（锚点）。-1 表示没在选。 */
    private var selectAnchor = -1

    /** 这一个选字手势**累计**选了几个字。 */
    private var selectSteps = 0

    /**
     * 退格上滑选字：把选区往左扩 `delta` 个字。
     *
     * 第一次调用记下**锚点**——那会儿光标还在手势开始的位置，之后按
     * 「锚点 + 累计格数」设选区。松手会来一条退格命令，那条会把锚点清掉。
     *
     * 用 `setSelection` 而不是 `SHIFT + 方向键`：方向键在安卓上是**焦点导航**用的，
     * 选到头会把焦点挪到界面按钮上（移光标已经踩过这个坑）。
     */
    private fun selectChars(connection: InputConnection, delta: Int) {
        if (selectSteps == 0) {
            val at = connection.getExtractedText(ExtractedTextRequest(), 0)?.selectionEnd ?: -1
            if (at < 0) {
                Log.w(TAG, "应用不吐光标位置，这一次选不了")
                return
            }
            selectAnchor = at
        }
        selectSteps += delta
        val target = (selectAnchor + selectSteps).coerceAtLeast(0)
        connection.setSelection(minOf(selectAnchor, target), maxOf(selectAnchor, target))
    }

    /**
     * 把光标按字符挪 `steps` 格（正数往右）。
     *
     * **不能用方向键**：`KEYCODE_DPAD_*` 在安卓上本来就是**焦点导航**用的，
     * 光标已经在头 / 尾时那个键没人消费，就会往上冒、把焦点挪到界面上的按钮去
     * （真机上「滑空格把焦点滑到返回 / 删除键上」就是这么来的）。
     *
     * 这里问出光标现在在哪儿，直接算好目标位置 `setSelection`——一步到位，
     * 也不会外溢成焦点移动。**只动这个输入框里的文本，碰不到界面上的别的东西。**
     */
    private fun moveCursor(connection: InputConnection, steps: Int) {
        val extracted = connection.getExtractedText(ExtractedTextRequest(), 0)
        if (extracted == null || extracted.selectionStart < 0) {
            Log.w(TAG, "应用不吐光标位置，这一次移不了")
            return
        }
        val length = extracted.text?.length ?: Int.MAX_VALUE
        val start = (extracted.selectionStart + steps).coerceIn(0, length)
        val end = (extracted.selectionEnd + steps).coerceIn(0, length)
        connection.setSelection(minOf(start, end), maxOf(start, end))
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

    /** 键预览气泡的浮动小窗。位图由 Rust 画，这里只贴上去。 */
    private var popup: KeyPopup? = null

    /**
     * 告诉 Rust 该画多宽，并把整块输入视图的高度要回来。
     *
     * 宽度**优先用视图量出来的那个**，不是屏幕宽：输入法窗口不一定占满屏幕——
     * 横屏时系统会给挖孔 / 手势区让出边上一条（实测 720×1280 的机器横过来之后
     * 窗口只从 x=136 起、宽 1144）。照屏幕宽画，键盘会宽出窗口、右边被切掉。
     *
     * 视图还没量出来时（首次调用）才退回屏幕宽；量出来之后 `onSizeChanged`
     * 会再叫一次这里，那时就是准的。
     */
    private fun configure(view: QingjianSurfaceView) {
        if (handle == 0L) return
        val metrics = resources.displayMetrics
        val width = if (view.width > 0) view.width else metrics.widthPixels
        QingjianNative.configure(
            handle,
            width / metrics.density,
            metrics.density,
            view.bottomInsetPoints,
            isDark(),
            isLandscape(),
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
    /** 横屏。键要矮一截——横屏竖向空间少，还用竖屏那个高度会占掉半个屏幕。 */
    private fun isLandscape(): Boolean =
        resources.configuration.orientation == Configuration.ORIENTATION_LANDSCAPE

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
