package app.qingjian.android

import android.content.res.Configuration
import android.inputmethodservice.InputMethodService
import android.util.Log
import android.view.KeyEvent
import android.view.MotionEvent
import android.view.View
import java.io.File
import java.util.Locale

/**
 * 青简的输入法服务。
 *
 * 现在是 M3 的形态：能打字、能选词——敲字母出候选，点候选（或空格）上屏到应用里。
 * 键按下态、中 / 英切换、符号面板是 M4。
 *
 * 这里是青简唯一碰安卓输入框的地方：Rust 那边只产「要上屏的文本」与「要原样转发的按键」，
 * 用什么 API 送出去是这一层的事。
 */
class QingjianImeService : InputMethodService() {
    /** Rust 侧的会话句柄，0 表示没打开。 */
    private var handle = 0L

    override fun onCreate() {
        super.onCreate()

        val dictionary = File(filesDir, DICTIONARY)
        if (!dictionary.isFile) {
            Log.e(TAG, "词库不存在：${dictionary.absolutePath}")
            return
        }

        handle = QingjianNative.open(dictionary.absolutePath, Locale.getDefault().toLanguageTag())
        if (handle == 0L) {
            Log.e(TAG, "会话打开失败")
            return
        }
        Log.i(TAG, "会话已打开，词库 ${dictionary.length() / 1024} KB")
        checkEngine()
    }

    override fun onCreateInputView(): View {
        val view = QingjianSurfaceView(this)
        // 直接按屏幕宽度配一次，不等视图量出来——视图的初始高度是 0，安卓不会给 0 高的视图
        // 发尺寸变化回调，等它就成了死锁。宽度变了（转屏）时再走 onConfigure。
        configure(view)
        view.onConfigure = { configure(view) }
        view.onTouch = { action, x, y ->
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
            if (action == MotionEvent.ACTION_UP) {
                val hit = QingjianNative.lastTouched(handle)
                if (hit.isNotEmpty()) {
                    Log.i(TAG, "按了 $hit")
                }
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

    /** 引擎还活着吗——敲一段拼音看有没有候选。渲染器出问题时靠它区分「引擎坏了」还是「画不出来」。 */
    private fun checkEngine() {
        QingjianNative.clear(handle)
        for (letter in ENGINE_PROBE) {
            QingjianNative.push(handle, letter)
        }
        Log.i(TAG, "$ENGINE_PROBE 的候选：${QingjianNative.candidates(handle).replace('\n', ' ')}")
        QingjianNative.clear(handle)
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
        // 验收用：光看位图看不出候选对不对，把文本也打一份（M4 删）。
        // 只打头几个——完整词库下「shi」有五百个候选，全打出来日志没法看。
        val candidates = QingjianNative.candidates(handle).split('\n').filter { it.isNotEmpty() }
        val head = candidates.take(CANDIDATE_LOG_LIMIT).joinToString(" ")
        val rest = (candidates.size - CANDIDATE_LOG_LIMIT).takeIf { it > 0 }?.let { " 等 $it 个" }
        Log.i(TAG, "候选：${head.ifEmpty { "（空）" }}${rest ?: ""}")
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

        /** 引擎自检用的拼音。 */
        const val ENGINE_PROBE = "kaifa"

        /** 候选日志最多打几个（M3 临时件，M4 删）。 */
        const val CANDIDATE_LOG_LIMIT = 6
    }
}
