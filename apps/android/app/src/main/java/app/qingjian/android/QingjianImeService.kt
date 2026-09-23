package app.qingjian.android

import android.content.ClipDescription
import android.content.ClipboardManager
import android.content.Intent
import android.content.pm.PackageManager
import android.content.res.Configuration
import android.inputmethodservice.InputMethodService
import android.os.Build
import android.os.Handler
import android.os.Looper
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

    /**
     * 应用那边此刻有没有**我们镜像过去的组字区**。
     *
     * 拼音没了的时候要靠它决定收尾怎么收：有就得**撤掉**（见 [mirrorPreedit]），
     * 没有才用 `finishComposingText()`。上屏时 `commitText` 会把组字区一起换掉，也跟着清掉。
     */
    private var mirrored = false

    /** 学习数据心跳用的。与键盘上那两个节拍器（连发 50ms、候选条惯性 16ms）各走各的。 */
    private val handler = Handler(Looper.getMainLooper())

    /**
     * 学习数据的**兜底**落盘：键盘开着时每 [LEARNING_FLUSH_INTERVAL_MS] 刷一次。
     *
     * 主路径是收起键盘那一刻（见 [onFinishInput]），但 **BACK 收起键盘时那个回调不一定到**，
     * 只有 [onStartInputView] 是每次必到的——所以得有这个心跳兜着。
     * 与电脑版同一个规矩（`apps/macos/src/host/mod.rs` 的 `LEARNING_FLUSH_INTERVAL`）。
     *
     * 为什么不每次上屏就写：见 `QingjianNative.flushLearning`。
     */
    private val learningTicker = object : Runnable {
        override fun run() {
            if (handle != 0L) QingjianNative.flushLearning(handle)
            handler.postDelayed(this, LEARNING_FLUSH_INTERVAL_MS)
        }
    }

    /**
     * 云联想的轮询：**只在有请求在飞时跑**——掩码里还有 [QingjianNative.FLAG_PREDICTING]
     * 才有下一拍，结果拿到、或者等太久自己就停。
     *
     * 结果是**非阻塞取的**（发了请求立刻返回，答案得回来取），所以得有人一直问。
     * 节拍与电脑版一致（mac 的 `PredictMonitor` 是 50ms 一拍）。
     */
    private val predictionTicker = object : Runnable {
        override fun run() {
            val view = inputView ?: return
            if (handle == 0L) return
            afterInput(view, QingjianNative.pollPrediction(handle), SystemClock.elapsedRealtime())
        }
    }

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
            // 可写的数据目录：剪贴板历史这类要留住的东西落在这儿（词库与 emoji 是解出来的，
            // 读不到就重解一遍；这些丢了就真没了）
            filesDir.absolutePath,
        )
        if (handle == 0L) {
            Log.e(TAG, "会话打开失败")
            return
        }
        Log.i(TAG, "会话已打开，词库 ${dictionary.length() / 1024} KB")

        systemClipboard?.addPrimaryClipChangedListener(clipboardListener)
    }

    /** 系统剪贴板。剪贴板页那份历史就是从这儿来的。 */
    private val systemClipboard: ClipboardManager?
        get() = getSystemService(ClipboardManager::class.java)

    /**
     * 复制了新东西：交给 Rust 记一条（去重、条数上限都在那边）。
     *
     * **敏感内容与空白不报**：密码管理器复制的会打 `EXTRA_IS_SENSITIVE` 标记，
     * 那种东西不该躺在输入法的历史里；空白记下来也没用。
     * 回调不一定在主线程上，所以挪到视图那条线程再碰会话（Rust 那边要求单线程）。
     */
    private val clipboardListener = ClipboardManager.OnPrimaryClipChangedListener {
        val view = inputView ?: return@OnPrimaryClipChangedListener
        val text = readClipboard() ?: return@OnPrimaryClipChangedListener
        view.post {
            // 排到这儿时输入法可能已经收摊了（`onDestroy` 把 handle 清零）
            if (handle == 0L) return@post
            val started = SystemClock.elapsedRealtime()
            val flags = QingjianNative.clipboardChanged(handle, text)
            afterInput(view, flags, started)
        }
    }

    /** 当前剪贴板里的文字；没有、不是文字、或者标了敏感时是 `null`。 */
    private fun readClipboard(): String? {
        val clip = systemClipboard?.primaryClip ?: return null
        if (clip.description.extras?.getBoolean(ClipDescription.EXTRA_IS_SENSITIVE) == true) {
            return null
        }
        val text = clip.getItemAt(0)?.coerceToText(this)?.toString() ?: return null
        return text.ifBlank { null }
    }

    /**
     * 把随包的资源解到应用私有目录，返回文件；解不出来返回 null。
     *
     * 标记文件记的是**这个 APK 是什么时候装的**（`lastUpdateTime`），所以升级一次就自动重解一遍，
     * 不用维护版本号。字体 10 MB，不这么记的话每次启动都要白拷。
     *
     * `name` 可以带子目录（`dicts/medicine.qj`）。标记文件**不跟着建子目录**，
     * 把斜杠换成下划线摊在 `filesDir` 根上——那只是一张便签，没必要跟着目录结构走。
     *
     * `required = false` 的是**可选**资源（英文词表、语言模型、释义表、领域词库）：
     * 打包时找不到源文件就不会进包，那是正常情况，日志降一档别吓人，
     * 调用方也不该因此放弃别的资源。
     */
    private fun ensureBundled(name: String, dir: File = filesDir, required: Boolean = true): File? {
        val target = File(dir, name)
        val stamp = File(filesDir, ".${name.replace('/', '_')}.installed")
        val revision = installedAt()
        if (target.isFile && stamp.isFile && stamp.readText() == revision) {
            return target
        }
        return try {
            target.parentFile?.mkdirs()
            assets.open(name).use { input ->
                target.outputStream().use { output -> input.copyTo(output) }
            }
            stamp.writeText(revision)
            target
        } catch (error: IOException) {
            if (required) {
                Log.e(TAG, "随包资源 $name 解不出来", error)
            } else {
                Log.w(TAG, "可选的随包资源 $name 不在包里，少一块功能", error)
            }
            null
        }
    }

    /** 随包资源（emoji 字体与 emoji 表、英文词表、语言模型、释义表、领域词库），解到一个目录里交给 Rust。 */
    private fun ensureExtras(): File? {
        val dir = File(filesDir, BUNDLE_DIR)
        for (name in BUNDLE_ASSETS) {
            if (ensureBundled(name, dir) == null) {
                return null
            }
        }
        // 英文词表、语言模型、释义表、领域词库**单独解、失败了也不拦**：APK 里没有它们
        // （打包时找不到源文件）不该把 emoji 一起拖下水——那边是键盘能不能画出来的事，
        // 这些只是少一块功能（英文模式退回直输 / 整句退化成一元词频 / 候选条不画译文 /
        // 专业词查不到）。解不出来就算了，Rust 那边读不到自然退。
        for (name in OPTIONAL_ASSETS) {
            ensureBundled(name, dir, required = false)
        }
        // 领域词库是**一个子目录**，有几本解几本：名字不写死在这儿，
        // 打包时往里放几本这里就跟着解几本（`assets.list` 直接问包里有啥）。
        for (name in assets.list(DICTS_ASSET_DIR).orEmpty()) {
            ensureBundled("$DICTS_ASSET_DIR/$name", dir, required = false)
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
        // 送数据与收尾**分开**：`ACTION_MOVE` 会按根逐次调 `onTouch`，收尾只在
        // `onTouchDone` 里做一次——不然两根手指移动就是整屏重画两遍（2026-09-23 修的）。
        view.onTouch = { action, pointer, x, y ->
            touchStartedAt = SystemClock.elapsedRealtime()
            QingjianNative.touch(handle, action, pointer, x, y)
        }
        view.onTouchDone = { flags -> afterInput(view, flags, touchStartedAt) }
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
        // 抬手速度：壳只管把安卓量到的速度报上去，甩不甩、甩多远是 Rust 的事。
        view.onFling = { pointer, velocityX, velocityY ->
            val started = SystemClock.elapsedRealtime()
            val flags = QingjianNative.fling(handle, pointer, velocityX, velocityY)
            afterInput(view, flags, started)
        }
        // 惯性的一帧：走多少由 Rust 按「过去多久」算。掩码里还有 FLAG_FLING 就接着敲。
        view.onFlingTick = { dt ->
            val started = SystemClock.elapsedRealtime()
            val flags = QingjianNative.flingStep(handle, dt)
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
    /** 这一批触摸是什么时候开始的——`onTouch` 与 `onTouchDone` 两次回调之间递这个。 */
    private var touchStartedAt = 0L

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
        // 用户点了工具页的「设置」：把键盘收起来、开设置页。
        // **显式收，不指望系统按焦点自己收**——部分 ROM 会把新 Activity 挤在键盘下面、或者它拿不到焦点。
        if (flags and QingjianNative.FLAG_SETTINGS != 0 && QingjianNative.takeSettings(handle)) {
            openSettings()
        }
        // 还在滑就按帧接着敲，滑完了就停。**只有这里知道 Rust 那边还在不在跑**，
        // 所以帧的开关也在这儿翻（惯性那几帧跟打字一样走这条收尾，慢了同样会报出来）。
        view.setFlinging(flags and QingjianNative.FLAG_FLING != 0)
        // 云联想有请求在飞就按拍子问；**没在飞时一个定时器都不跑**（E7 定的那条）。
        // 先撤再排：每敲一键都重置节拍，等结果这一段时间里不必那么急。
        handler.removeCallbacks(predictionTicker)
        if (flags and QingjianNative.FLAG_PREDICTING != 0) {
            handler.postDelayed(predictionTicker, PREDICTION_POLL_MS)
        }
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
     * 打开设置页（键盘工具页那一格）。
     *
     * Service 不是 Activity，拉起一个 Activity 必须带 `NEW_TASK`。
     * Android 12 起对「后台启动 Activity」有限制，但输入法窗口此刻是可见的前台窗口，属于豁免情形。
     *
     * 设置页**只跟配置文件打交道、不碰会话句柄**——这边随时可能在 `onDestroy` 里把会话关掉，
     * 那边拿着 handle 就是野指针。改完也不用通知谁：用户返回时键盘重弹，
     * [onStartInputView] 里读一遍文件就生效了。
     */
    private fun openSettings() {
        requestHideSelf(0)
        startActivity(
            Intent(this, QingjianSettingsActivity::class.java)
                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP)
        )
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
        var clear = false
        QingjianNative.takeCommands(handle)?.forEach { code ->
            when (code) {
                QingjianNative.COMMAND_MOVE_LEFT -> cursor -= 1
                QingjianNative.COMMAND_MOVE_RIGHT -> cursor += 1
                QingjianNative.COMMAND_CLEAR_ALL -> clear = true
                else -> {
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
        if (clear) clearToStart(connection)
        QingjianNative.takeCommit(handle)?.let {
            connection.commitText(it, 1)
            // `commitText` 会把组字区一起换掉，应用那边已经没有我们镜像过去的拼音了
            mirrored = false
        }
    }

    /**
     * 把**光标前面整段**清掉。⌫ 上往上滑、松手时兑现。
     *
     * 用 `deleteSurroundingText(前面有几个字, 0)` 一次删完——这是安卓专门干这事的 API：
     * 不用自己算选区、不会外溢成焦点移动、也不碰界面上的别的东西。
     */
    private fun clearToStart(connection: InputConnection) {
        val before = connection.getExtractedText(ExtractedTextRequest(), 0)?.selectionStart ?: -1
        if (before <= 0) return
        connection.deleteSurroundingText(before, 0)
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

    /**
     * 把拼音镜像到输入框；空串表示这一串没了，**应用那边镜像过去的也得撤掉**。
     *
     * **`finishComposingText()` 不能当「清空」用**：它的语义是「组字到此为止，**文字留在原处**」
     * ——只去掉那层下划线，一个字都不删。拿它收尾，拼音就被**烘焙成了正式文本**：
     * 输入法这边缓冲区已经空了、应用那边却多出一串删不掉的。用户看到的正是这个
     * （「ni 打错了要按好几下退格才干净」）：退格删掉的是输入法的拼音，那一串留在原地。
     *
     * 撤掉要用 `setComposingText("", 0)`——把组字区**替换成空**，也就是删掉。
     */
    private fun mirrorPreedit() {
        val connection = currentInputConnection ?: return
        val preedit = QingjianNative.takePreedit(handle)
        if (preedit.isNotEmpty()) {
            connection.setComposingText(preedit, 1)
            mirrored = true
            return
        }
        if (mirrored) {
            connection.setComposingText("", 0)
            mirrored = false
        }
        connection.finishComposingText()
    }

    /**
     * 返回键：先把输入法**自己开着的那层**收掉（展开选词的面板、工具页这些），
     * 收掉了这一下就不再往下传。
     *
     * Rust 那边说「这一下归我管」才吃（返回非 0 的掩码）；什么都没开着就返回 0，
     * 返回键照常交给应用收起键盘——那是用户熟悉的动作，输入法不该抢。
     *
     * 放在 `onKeyDown` 而不是 `onKeyUp`：`onKeyUp` 到时系统多半已经把窗口收了。
     */
    override fun onKeyDown(keyCode: Int, event: KeyEvent): Boolean {
        if (keyCode == KeyEvent.KEYCODE_BACK && handle != 0L) {
            val flags = QingjianNative.dismiss(handle)
            if (flags != 0) {
                inputView?.let { afterInput(it, flags, SystemClock.elapsedRealtime()) }
                return true
            }
        }
        return super.onKeyDown(keyCode, event)
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
        // 键盘弹出来了，学习数据的兜底心跳开起来（收起时在 `onFinishInput` 里停）。
        // 先撤再排：`onStartInputView` 有可能连着来两次而中间没有 `onFinishInput`，不撤会排两条。
        handler.removeCallbacks(learningTicker)
        handler.postDelayed(learningTicker, LEARNING_FLUSH_INTERVAL_MS)
        // 补一次当前剪贴板：上面那个监听器只管**变化**，输入法起来之前复制的东西收不到。
        // 放在这儿而不是 `onCreate`——那会儿输入法窗口还没显示，
        // 非前台读剪贴板会让系统弹一条「某某读取了剪贴板」的提示（Android 12 起）。
        readClipboard()?.let { QingjianNative.clipboardChanged(handle, it) }
        // 密码框里不学、不记、**不发云端**——判定在壳这边（引擎那边另有一道闸）。
        // 每次弹键盘都要重报一遍：换了个输入框就得重新判。
        QingjianNative.setPrivate(handle, info.isPrivate())
        // 云联想拿光标前后的文本当上下文，也是每次弹键盘取一次
        refreshSurrounding()
        // 配置文件变了就重读并应用（用户从设置页回来时必走这一条）。
        // **就挂在这儿，不另排心跳**：键盘每次弹出来这个回调必到，而它是唯一一定到的
        // （BACK 收起键盘时 onFinishInput 不触发，2026-09-22 实测过）。桌面要每秒轮询
        // 是因为它没有「键盘弹出」这个事件，安卓有就不该白养一个定时器。
        // 没变时它返回 0，底下的重画一次都不会发生。
        var flags = QingjianNative.configPoll(handle)
        flags = flags or QingjianNative.resetPanel(handle)
        // 震动那两档是壳的事（Rust 不管手感），从同一份配置里读一遍再交给 KeyFeedback。
        // 键盘弹一次读一次文件，跟「每敲一下读一次」是两回事。
        applyVibrationConfig(QingjianNative.configRead(filesDir.absolutePath))
        inputView?.let { view ->
            if (flags and QingjianNative.FLAG_BAR != 0) refreshBar(view)
            if (flags and QingjianNative.FLAG_KEYBOARD != 0) refreshKeyboard(view)
        }
    }

    /**
     * 键盘窗口藏起来了：**这才是学习数据落盘的主路径**。
     *
     * 原以为 `onFinishInput` 管这事，2026-09-22 在模拟器上实测**不是**：按 BACK 收起键盘时
     * 那条回调压根没来（`ImeTracker` 只报了 `HIDE_SOFT_INPUT_BY_BACK_KEY`），
     * 数据最后是等 60 秒心跳兜下来的——最坏要多等一分钟，而输入法进程随时可能被杀。
     * `onWindowHidden` 是窗口真藏起来时必到的，落在这儿才跟得上。
     *
     * 与 [`onFinishInput`] 都留着：那个管「焦点走了但窗口还在」，这个管「窗口收了」。
     * 重复调不要紧——没有脏数据时落盘是空操作。
     */
    override fun onWindowHidden() {
        super.onWindowHidden()
        if (handle == 0L) return
        handler.removeCallbacks(learningTicker)
        handler.removeCallbacks(predictionTicker)
        QingjianNative.flushLearning(handle)
    }

    override fun onFinishInput() {
        super.onFinishInput()
        if (handle == 0L) return
        // 焦点离开输入框也会走到这儿（切换应用、点到别处）。与 `onWindowHidden` 一样落一次盘，
        // 没有脏数据时是空操作，所以不必判断谁先谁后。
        handler.removeCallbacks(learningTicker)
        handler.removeCallbacks(predictionTicker)
        QingjianNative.flushLearning(handle)
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
        val density = metrics.density
        val width = if (view.width > 0) view.width else metrics.widthPixels
        QingjianNative.configure(
            handle,
            width / density,
            screenHeightPoints(metrics.widthPixels, metrics.heightPixels, density),
            density,
            view.bottomInsetPoints,
            isDark(),
            isLandscape(),
        )
        // 高度不用自己算：视图按两张位图加起来的像素高自己量
        refreshBar(view)
        refreshKeyboard(view)
    }

    /**
     * 屏幕在**当前方向**上的高度（点）：竖屏取长边、横屏取短边。键盘高度按它算。
     *
     * **不直接用 `displayMetrics.heightPixels`**：有的 ROM 转屏之后它还是报竖屏那个值，
     * 那样横屏会按八百点去算，算出一块占掉半个屏幕的键盘。按长边 / 短边分则与转没转屏无关。
     */
    private fun screenHeightPoints(widthPixels: Int, heightPixels: Int, density: Float): Float {
        val long = maxOf(widthPixels, heightPixels) / density
        val short = minOf(widthPixels, heightPixels) / density
        return if (isLandscape()) short else long
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

    /**
     * 这个输入框是不是密码框。
     *
     * 按 `inputType` 的**分类位 + 变体位**判：密码那几种都藏在 variation 里，得掩码取出来比。
     * 「看得见的密码」（`VISIBLE_PASSWORD`）也算——它照样是密码。
     */
    private fun EditorInfo?.isPrivate(): Boolean {
        val info = this ?: return false
        val kind = info.inputType and EditorInfo.TYPE_MASK_CLASS
        val variation = info.inputType and EditorInfo.TYPE_MASK_VARIATION
        return (kind == EditorInfo.TYPE_CLASS_TEXT && variation in TEXT_PASSWORDS) ||
            (
                kind == EditorInfo.TYPE_CLASS_NUMBER &&
                    variation == EditorInfo.TYPE_NUMBER_VARIATION_PASSWORD
                )
    }

    /**
     * 把光标前后的文本报给 Rust（云联想拿它当上下文）。
     *
     * **跨进程搬一长段太亏**，先截到几百字符——引擎那边还会按 `[predict] lookback / lookahead` 再裁一次。
     * 取不到（有些应用不给）就当空串，联想照样会发，只是上下文少一点。
     */
    private fun refreshSurrounding() {
        val connection = currentInputConnection ?: return
        val before = connection.getTextBeforeCursor(SURROUNDING_CHARS, 0)?.toString().orEmpty()
        val after = connection.getTextAfterCursor(SURROUNDING_CHARS, 0)?.toString().orEmpty()
        QingjianNative.setSurrounding(handle, before, after)
    }

    /** 系统现在是深色吗。 */
    /** 横屏。键要矮一截——横屏竖向空间少，还用竖屏那个高度会占掉半个屏幕。 */
    private fun isLandscape(): Boolean =
        resources.configuration.orientation == Configuration.ORIENTATION_LANDSCAPE

    private fun isDark(): Boolean =
        (resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK) ==
            Configuration.UI_MODE_NIGHT_YES

    override fun onDestroy() {
        systemClipboard?.removePrimaryClipChangedListener(clipboardListener)
        handler.removeCallbacks(learningTicker)
        handler.removeCallbacks(predictionTicker)
        if (handle != 0L) {
            // 落盘**必须在 `close` 之前**：`close` 一调会话对象就没了，之后再叫它只会拿到空句柄
            QingjianNative.flushLearning(handle)
            QingjianNative.close(handle)
            handle = 0L
        }
        super.onDestroy()
    }

    /**
     * `internal` 而不是 `private`：设置页要问 [`BUNDLE_DIR`]（词库在它下面），
     * 那个字符串只该有这一份。
     */
    internal companion object {
        const val TAG = "Qingjian"

        /** 学习数据兜底落盘的间隔，与电脑版一致（`LEARNING_FLUSH_INTERVAL`）。 */
        const val LEARNING_FLUSH_INTERVAL_MS = 60_000L

        /** 云联想轮询的节拍（毫秒）。与电脑版一致（mac 的 `PredictMonitor` 是 50ms 一拍）。 */
        const val PREDICTION_POLL_MS = 50L

        /** 报给云联想的光标前后文最多各取多少字符。引擎那边还会按 `[predict]` 的配置再裁。 */
        const val SURROUNDING_CHARS = 256

        /** 算「这是不是密码框」用的那几种 variation（`EditorInfo.inputType` 的变体位）。 */
        val TEXT_PASSWORDS = intArrayOf(
            EditorInfo.TYPE_TEXT_VARIATION_PASSWORD,
            EditorInfo.TYPE_TEXT_VARIATION_VISIBLE_PASSWORD,
            EditorInfo.TYPE_TEXT_VARIATION_WEB_PASSWORD,
        )

        /** 随包词库的文件名，放在应用私有目录。 */
        const val DICTIONARY = "dict.qj"

        /**
         * **必须**解出来的随包资源（在 APK 的 assets 里，启动时解到私有目录的 [BUNDLE_DIR]）：
         * emoji 字体与 emoji 表。解不出来键盘就画不出表情，所以里面任何一个失败都整个放弃。
         */
        val BUNDLE_ASSETS = listOf(
            "NotoColorEmoji.ttf",
            "emoji-zh.tsv",
            "emoji-en.tsv",
            // 表情面板的两张表（emoji 与颜文字各一张）
            "emoji-panel.tsv",
            "kaomoji-panel.tsv",
        )

        /** 英文词表。**可选的**——没有它英文模式退回直输，见 [ensureExtras]。 */
        const val ENGLISH_ASSET = "english.tsv"

        /** 语言模型（44 MB）。**可选的**——没有它整句退化成一元词频，见 [ensureExtras]。 */
        const val LANGUAGE_MODEL_ASSET = "lm.qj"

        /**
         * 领域词库在 assets 里的子目录（11 本：成语 / 医学 / 法律 / 地名 …）。
         *
         * 单独一个子目录是因为 assets 根上已经摊着词库、词表、模型、释义表了。
         * **有几本解几本**，名字不写死——见 [ensureExtras]。
         */
        const val DICTS_ASSET_DIR = "dicts"

        /**
         * 可选资源清单：解不出来只是少一块功能，不该拦下 emoji 那几个必需的。
         *
         * 释义表四本（`zh` 英→中 + 三本学习语言）都在这儿——没有它候选条就是光秃秃的词。
         */
        val OPTIONAL_ASSETS = listOf(
            ENGLISH_ASSET,
            LANGUAGE_MODEL_ASSET,
            "glossary-zh.qj",
            "glossary-en.qj",
            "glossary-ja.qj",
            "glossary-es.qj",
        )

        /** 随包那几个数据文件解到私有目录时用的子目录名（emoji、颜文字、英文词表都在里头）。 */
        const val BUNDLE_DIR = "bundle"

        /** 一次触摸超过这么多毫秒就报一声（约一帧）；打字手感的分水岭。 */
        const val SLOW_TOUCH_MS = 16L
    }
}
