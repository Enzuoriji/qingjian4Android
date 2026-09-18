package app.qingjian.android

import android.content.pm.PackageManager
import android.content.res.Configuration
import android.inputmethodservice.InputMethodService
import android.os.Build
import android.os.SystemClock
import android.util.Log
import android.view.KeyEvent
import android.view.View
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
        // 直接按屏幕宽度配一次，不等视图量出来——视图的初始高度是 0，安卓不会给 0 高的视图
        // 发尺寸变化回调，等它就成了死锁。宽度变了（转屏）时再走 onConfigure。
        configure(view)
        view.onConfigure = { configure(view) }
        view.onTouch = { action, x, y ->
            // 打出慢帧：这一整套（引擎查询 + 画两张位图 + 过 JNI + 传成 Bitmap）都在
            // 触摸回调里同步做，一次超过一帧的时间打字就会跟不上手感。慢了就报出来。
            val started = SystemClock.elapsedRealtime()
            val flags = QingjianNative.touch(handle, action, x, y)
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
            val elapsed = SystemClock.elapsedRealtime() - started
            if (elapsed >= SLOW_TOUCH_MS) {
                Log.w(TAG, "这一下花了 ${elapsed}ms，打字会跟不上手感")
            }
        }
        return view
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
    override fun onFinishInput() {
        super.onFinishInput()
        if (handle == 0L) return
        QingjianNative.clear(handle)
        mirrorPreedit()
    }

    /** 按当前屏幕宽度告诉 Rust 该画多宽，并把整块输入视图的高度要回来。 */
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

    /** 重新取一张候选条位图贴上。Rust 那边没脏就会返回同一张，不会白画。 */
    private fun refreshBar(view: QingjianSurfaceView) {
        if (handle == 0L) return
        val bytes = QingjianNative.barSurface(handle)
        if (bytes == null || bytes.isEmpty()) {
            Log.e(TAG, "候选条没画出来（渲染器没建起来，或者还没配过宽度）")
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
