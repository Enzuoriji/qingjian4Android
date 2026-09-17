package app.qingjian.android

import android.inputmethodservice.InputMethodService
import android.util.Log
import android.view.View
import android.widget.LinearLayout
import android.widget.TextView
import java.io.File

/**
 * 青简的输入法服务。
 *
 * 现在是 Phase 1 的形态：不接按键，启动时自己喂一段拼音再把候选显示出来，
 * 用来验证 Rust 引擎在安卓上真的能加载词库、查出候选。接了软键盘之后这段自检要去掉。
 */
class QingjianImeService : InputMethodService() {
    /** Rust 侧的会话句柄，0 表示没打开。 */
    private var handle = 0L

    /** 显示自检结果的文本视图。 */
    private var output: TextView? = null

    override fun onCreate() {
        super.onCreate()

        val dictionary = File(filesDir, DICTIONARY)
        if (!dictionary.isFile) {
            Log.e(TAG, "词库不存在：${dictionary.absolutePath}")
            return
        }

        handle = QingjianNative.open(dictionary.absolutePath)
        if (handle == 0L) {
            Log.e(TAG, "会话打开失败")
        } else {
            Log.i(TAG, "会话已打开，词库 ${dictionary.length() / 1024} KB")
        }
    }

    override fun onCreateInputView(): View {
        val layout = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(32, 32, 32, 32)
        }

        output = TextView(this).apply {
            textSize = 16f
            layout.addView(this)
        }

        return layout
    }

    override fun onStartInputView(info: android.view.inputmethod.EditorInfo?, restarting: Boolean) {
        super.onStartInputView(info, restarting)
        selfTest()
    }

    /** 自己喂一段拼音，把候选显示出来——Phase 1 验证的就是这条链路。 */
    private fun selfTest() {
        if (handle == 0L) {
            show("引擎没打开，看日志")
            return
        }

        QingjianNative.clear(handle)
        for (letter in PROBE) {
            QingjianNative.push(handle, letter)
        }

        val candidates = QingjianNative.candidates(handle)
        Log.i(TAG, "$PROBE 的候选：$candidates")
        show("$PROBE →\n$candidates")
    }

    private fun show(text: String) {
        output?.text = text
    }

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

        /** 自检用的拼音。 */
        const val PROBE = "kaifa"
    }
}
