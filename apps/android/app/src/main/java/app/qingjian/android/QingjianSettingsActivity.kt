package app.qingjian.android

import android.app.Activity
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import android.view.View
import android.view.ViewGroup
import android.view.WindowInsets
import android.widget.Button
import android.widget.CheckBox
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.RadioButton
import android.widget.RadioGroup
import android.widget.SeekBar
import android.widget.Switch
import android.widget.TextView
import android.widget.Toast
import org.json.JSONArray
import org.json.JSONObject
import java.io.File

/**
 * 设置页：**原生控件**，不引 AndroidX（`docs/contributing.md` 的「显示面自绘、控件面原生」）。
 *
 * 这一页**只跟 `config.toml` 打交道，不碰会话句柄**——输入法服务随时可能被销毁，
 * 那时手里那个 handle 就是野指针。改完不用通知任何人：用户返回应用时键盘会重弹一次，
 * 输入法服务在 `onStartInputView` 里读一遍文件就生效了（见 `QingjianNative.configPoll`）。
 *
 * **控件就是配置文件的一面镜子**：每次进来都重新读一遍文件再填控件，不在这一页里
 * 另存一份状态。写失败就把值回滚（重新读一遍），不静默吞。
 */
class QingjianSettingsActivity : Activity() {
    /** 配置文件所在的目录。与输入法服务是同一份——`filesDir` 是按应用给的，不由组件决定。 */
    private val dataDir: String get() = filesDir.absolutePath

    /** 随包资源目录（词库在它下面的 `dicts/` 里）。与输入法服务解到的是同一个地方。 */
    private val bundleDir: String
        get() = File(filesDir, QingjianImeService.BUNDLE_DIR).absolutePath

    /** 从文件读到的配置。控件按它填，改完也回写到这里。 */
    private var config: JSONObject? = null

    /** 「测试连接」问结果用的节拍器（与输入法那边等云联想是同一个路数）。 */
    private val handler = Handler(Looper.getMainLooper())

    /** 这次测试是什么时候起的——过了上限就别等了。 */
    private var testStartedAt = 0L

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_settings)
        applyInsets(findViewById(R.id.settings_root))
        if (!load()) return
        bind()
    }

    /**
     * 把系统栏那块地方让出来。
     *
     * targetSdk 35 起应用默认 edge-to-edge，不让的话状态栏会盖在第一行标题上。
     * 没有 AndroidX 就没有 `ViewCompat.setOnApplyWindowInsetsListener`，用 framework 自带的这个。
     */
    private fun applyInsets(root: View) {
        root.setOnApplyWindowInsetsListener { view, insets ->
            val top: Int
            val bottom: Int
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
                val bars = insets.getInsets(WindowInsets.Type.systemBars())
                top = bars.top
                bottom = bars.bottom
            } else {
                @Suppress("DEPRECATION")
                top = insets.systemWindowInsetTop
                @Suppress("DEPRECATION")
                bottom = insets.systemWindowInsetBottom
            }
            view.setPadding(0, top, 0, bottom)
            insets
        }
    }

    /**
     * 从文件读一份配置。读不出来就把错误摆出来、**别的控件一个都不建**——
     * 不替用户「修好」那个文件（桌面两壳也是这条规矩：改坏了自己改回来，
     * 我们一动手他的注释和顺序就没了）。
     */
    private fun load(): Boolean {
        val envelope = QingjianNative.configRead(dataDir)
            ?.let { runCatching { JSONObject(it) }.getOrNull() }
        if (envelope == null || !envelope.optBoolean("ok")) {
            val reason = envelope?.optString("error")?.takeIf { it.isNotEmpty() } ?: "读不出来"
            findViewById<TextView>(R.id.settings_error).apply {
                visibility = View.VISIBLE
                text = getString(R.string.settings_broken, reason)
            }
            return false
        }
        config = envelope.getJSONObject("config")
        return true
    }

    /** 按 [config] 填控件。控件一变就写文件（见 [save]）。 */
    private fun bind() {
        val config = this.config ?: return
        val general = config.getJSONObject("general")
        val keyboard = config.getJSONObject("keyboard")
        val predict = config.getJSONObject("predict")
        val fuzzy = config.getJSONObject("fuzzy")
        val dictionaries = config.getJSONObject("dictionaries")

        radioGroup(
            R.id.learning_language,
            LANGUAGES,
            general.optString("learning_language", "en"),
        ) { save("general", "learning_language", it) }

        radioGroup(
            R.id.vibration,
            VIBRATIONS,
            keyboard.optString("vibration", "click"),
        ) { save("keyboard", "vibration", it) }

        slider(
            R.id.vibration_ms,
            R.id.vibration_ms_label,
            keyboard.optInt("vibration_ms", 20),
            MIN_VIBRATION_MS,
            MAX_VIBRATION_MS,
            R.string.settings_vibration_ms_value,
        ) { save("keyboard", "vibration_ms", it) }

        checkGrid(
            findViewById(R.id.fuzzy_grid),
            FUZZY,
            2,
            { key -> fuzzy.optBoolean(key, false) },
        ) { key, checked -> save("fuzzy", key, checked) }

        bindDictionaries(dictionaries)

        switch(R.id.predict_enabled, predict.optBoolean("enabled", false)) {
            save("predict", "enabled", it)
        }
        text(R.id.predict_base_url, predict.string("base_url"))
        text(R.id.predict_model, predict.string("model"))
        text(R.id.predict_api_key, predict.string("api_key"))
        slider(
            R.id.predict_slots,
            R.id.predict_slots_label,
            predict.optInt("slots", 2),
            0,
            9,
            R.string.settings_cloud_slots_value,
        ) { save("predict", "slots", it) }
        switch(R.id.predict_sentence, predict.optBoolean("sentence", true)) {
            save("predict", "sentence", it)
        }
        findViewById<Button>(R.id.predict_test).setOnClickListener { startCloudTest() }
    }

    /**
     * 「测试连接」：起一次最小探活，然后按拍子问结果。
     *
     * 与输入法那边等云联想是**同一个路数**（发起 + 轮询）——网络那一层本身就是非阻塞的。
     * 问的是**文件里当前那份**配置，所以刚填完密钥直接点就行（这一页改一项写一项）。
     */
    private fun startCloudTest() {
        val status = findViewById<TextView>(R.id.predict_status)
        val error = QingjianNative.cloudTestStart(dataDir)
        if (!error.isNullOrEmpty()) {
            status.text = error
            return
        }
        status.setText(R.string.settings_cloud_testing)
        testStartedAt = SystemClock.elapsedRealtime()
        pollCloudTest()
    }

    /** 问一次结果；还没回来就接着排下一拍，**过了上限就收摊**（与电脑版一样 30 秒）。 */
    private fun pollCloudTest() {
        val status = findViewById<TextView>(R.id.predict_status)
        val result = QingjianNative.cloudTestPoll()
            ?.let { runCatching { JSONObject(it) }.getOrNull() }
            ?: return
        if (!result.optBoolean("done")) {
            if (SystemClock.elapsedRealtime() - testStartedAt >= CLOUD_TEST_TIMEOUT_MS) {
                status.setText(R.string.settings_cloud_test_timeout)
                return
            }
            handler.postDelayed({ pollCloudTest() }, CLOUD_TEST_POLL_MS)
            return
        }
        status.text = result.optString("text")
    }

    override fun onDestroy() {
        handler.removeCallbacksAndMessages(null)
        super.onDestroy()
    }

    /**
     * 词库那 11 本：**名字不是写死的**，是问 Rust 要的（`QingjianNative.domainList`
     * 从每本词库的元数据里读中文名）。以后加一本词库，这一页跟着就有了。
     */
    private fun bindDictionaries(dictionaries: JSONObject) {
        val grid = findViewById<LinearLayout>(R.id.domain_grid)
        val books = QingjianNative.domainList(bundleDir)
            ?.let { runCatching { JSONArray(it) }.getOrNull() }
        if (books == null || books.length() == 0) {
            // 随包资源还没解出来（输入法没启用过）时就是这样：没得选，整组收起来
            grid.visibility = View.GONE
            return
        }
        val saved = dictionaries.optJSONArray("domains") ?: JSONArray()
        var enabled = (0 until saved.length()).map { saved.getString(it) }.toSet()
        for (index in 0 until books.length()) {
            val book = books.getJSONObject(index)
            val stem = book.getString("stem")
            val box = CheckBox(this).apply {
                text = book.optString("name", stem)
                isChecked = stem in enabled
                // 与上面那几组单选一样：占满整行，点名字右边的空白也该管用
                layoutParams = LinearLayout.LayoutParams(
                    ViewGroup.LayoutParams.MATCH_PARENT,
                    ViewGroup.LayoutParams.WRAP_CONTENT,
                )
            }
            box.setOnCheckedChangeListener { _, checked ->
                enabled = if (checked) enabled + stem else enabled - stem
                save("dictionaries", "domains", enabled.toList())
            }
            grid.addView(box)
        }
    }

    /** 一组单选。选中的那个是 `value` 里的键。 */
    private fun radioGroup(id: Int, options: List<Pair<String, Int>>, value: String, onChange: (String) -> Unit) {
        val group = findViewById<RadioGroup>(id)
        for ((key, label) in options) {
            val button = RadioButton(this).apply {
                // RadioGroup 靠 id 管互斥，不给 id 的话几个按钮会一起亮
                this.id = View.generateViewId()
                setText(label)
                isChecked = key == value
                // **占满整行**：默认是 wrap_content，那样只有点在那几个字上才有反应，
                // 点右边一大片空白没动静（2026-09-22 模拟器上就是这么点空的）
                layoutParams = RadioGroup.LayoutParams(
                    ViewGroup.LayoutParams.MATCH_PARENT,
                    ViewGroup.LayoutParams.WRAP_CONTENT,
                )
            }
            // 监听器**要在设完 isChecked 之后再挂**，否则填初始值这一下就会当成用户改的写一遍文件
            button.setOnCheckedChangeListener { _, checked -> if (checked) onChange(key) }
            group.addView(button)
        }
    }

    /** 一个滑块，右边那个标签跟着显示当前值。**松手才写文件**——拖着的时候每格都写太糟蹋。 */
    private fun slider(
        id: Int,
        labelId: Int,
        value: Int,
        min: Int,
        max: Int,
        format: Int,
        onChange: (Int) -> Unit,
    ) {
        val label = findViewById<TextView>(labelId)
        val bar = findViewById<SeekBar>(id)
        bar.max = max - min
        bar.setOnSeekBarChangeListener(object : SeekBar.OnSeekBarChangeListener {
            override fun onProgressChanged(bar: SeekBar, progress: Int, fromUser: Boolean) {
                label.text = getString(format, progress + min)
            }

            override fun onStartTrackingTouch(bar: SeekBar) = Unit

            override fun onStopTrackingTouch(bar: SeekBar) {
                onChange(bar.progress + min)
            }
        })
        bar.progress = value.coerceIn(min, max) - min
    }

    /**
     * 一组多选，每行 [columns] 个。`options` 是「配置键 → 显示的名字」，
     * 当前的勾选状态由 `checked` 问（它读的是文件里那份，不是控件的现状）。
     */
    private fun checkGrid(
        container: LinearLayout,
        options: List<Pair<String, String>>,
        columns: Int,
        checked: (String) -> Boolean,
        onChange: (String, Boolean) -> Unit,
    ) {
        for (row in options.chunked(columns)) {
            val line = LinearLayout(this).apply { orientation = LinearLayout.HORIZONTAL }
            for ((key, label) in row) {
                val box = CheckBox(this).apply {
                    text = label
                    isChecked = checked(key)
                    layoutParams = LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f)
                }
                box.setOnCheckedChangeListener { _, value -> onChange(key, value) }
                line.addView(box)
            }
            container.addView(line)
        }
    }

    private fun switch(id: Int, value: Boolean, onChange: (Boolean) -> Unit) {
        findViewById<Switch>(id).apply {
            isChecked = value
            setOnCheckedChangeListener { _, checked -> onChange(checked) }
        }
    }

    /** 一个输入框：**失焦时写**（外加 [onPause] 补一次，见那里）。 */
    private fun text(id: Int, value: String) {
        findViewById<EditText>(id).apply {
            setText(value)
            setOnFocusChangeListener { _, focused -> if (!focused) commitText(id) }
        }
    }

    /**
     * 取一个字符串键。
     *
     * **`optString` 不能直接用**：配置里可空的项（`api_key`）没填时 serde 写成 JSON 的 `null`，
     * 而 `optString` 遇到它会返回**字符串 `"null"`**——密钥框会预填一个假的「null」
     * （密码框里看着就是四个点），`onPause` 还可能把它写回配置文件。缺省值也走这条。
     */
    private fun JSONObject.string(key: String): String = if (isNull(key)) "" else optString(key, "")

    /**
     * 输入框失焦才写文件（每敲一个字写一遍太糟蹋），可用户完全可能改完直接按返回键——
     * 那一下不失焦，改动就丢了。所以暂停时补一次，**只写真的变了的那几个**。
     */
    override fun onPause() {
        super.onPause()
        commitText(R.id.predict_base_url)
        commitText(R.id.predict_model)
        commitText(R.id.predict_api_key)
    }

    /** 把某个输入框当前的值写进文件（与文件里那份一样就什么都不做）。 */
    private fun commitText(id: Int) {
        val field = findViewById<EditText>(id) ?: return
        val key = when (id) {
            R.id.predict_base_url -> "base_url"
            R.id.predict_model -> "model"
            else -> "api_key"
        }
        val current = field.text.toString()
        val saved = config?.optJSONObject("predict")?.string(key).orEmpty()
        if (current != saved) save("predict", key, current)
    }

    /**
     * 改一项写一项。写失败就**把这一页按文件重读一遍**（控件回到真正生效的值）并把原因摆出来——
     * 不静默吞，用户会以为改上了。
     */
    private fun save(section: String, key: String, value: Any) {
        val error = when (value) {
            is Boolean -> QingjianNative.configSetBool(dataDir, section, key, value)
            is Int -> QingjianNative.configSetInt(dataDir, section, key, value)
            is String -> QingjianNative.configSetString(dataDir, section, key, value)
            is List<*> -> QingjianNative.configSetArray(
                dataDir,
                section,
                key,
                JSONArray(value).toString(),
            )
            else -> return
        }
        if (!error.isNullOrEmpty()) {
            Toast.makeText(this, error, Toast.LENGTH_LONG).show()
            if (load()) bind()
            return
        }
        // 记下来，免得 onPause 里拿旧值再写一遍
        config?.optJSONObject(section)?.put(key, value)
    }

    private companion object {
        /** 「测试连接」问结果的节拍与上限（毫秒），与电脑版一致（mac 的 `CloudTestMonitor`）。 */
        const val CLOUD_TEST_POLL_MS = 200L
        const val CLOUD_TEST_TIMEOUT_MS = 30_000L

        /** 自定义震动的范围（毫秒），与 `qingjian_platform` 的 `MIN/MAX_VIBRATION_MS` 一致。 */
        const val MIN_VIBRATION_MS = 1
        const val MAX_VIBRATION_MS = 50

        /** 学习语言。**键要与 `Language::from_str` 认的代码一致**（`en` / `ja` / `es` / `off`）。 */
        val LANGUAGES = listOf(
            "en" to R.string.settings_language_en,
            "ja" to R.string.settings_language_ja,
            "es" to R.string.settings_language_es,
            "off" to R.string.settings_language_off,
        )

        /**
         * 按键震动的感觉。键与 `crates/qingjian-platform/src/config/keyboard.rs` 的
         * `VibrationStyle` 一一对应，**改一边必须同时改另一边**（跨语言没得共享）。
         */
        val VIBRATIONS = listOf(
            "off" to R.string.settings_vibration_off,
            "tick" to R.string.settings_vibration_tick,
            "click" to R.string.settings_vibration_click,
            "heavy" to R.string.settings_vibration_heavy,
            "double" to R.string.settings_vibration_double,
            "custom" to R.string.settings_vibration_custom,
        )

        /**
         * 模糊音那九条，键与 `qingjian_core::FuzzyRules::NAMES` 一致（也是配置里 `[fuzzy]` 的字段名）。
         * 两列一行。
         */
        val FUZZY = listOf(
            "z_zh" to "z ↔ zh",
            "c_ch" to "c ↔ ch",
            "s_sh" to "s ↔ sh",
            "n_l" to "n ↔ l",
            "f_h" to "f ↔ h",
            "l_r" to "l ↔ r",
            "an_ang" to "an ↔ ang",
            "en_eng" to "en ↔ eng",
            "in_ing" to "in ↔ ing",
        )
    }
}
