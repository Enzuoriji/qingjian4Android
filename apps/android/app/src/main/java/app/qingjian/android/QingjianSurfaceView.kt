package app.qingjian.android

import android.content.Context
import android.graphics.Bitmap
import android.graphics.Canvas
import android.os.SystemClock
import android.util.Log
import android.view.MotionEvent
import android.view.View
import android.view.WindowInsets

/** 按住多久开始连发（毫秒）。 */
private const val REPEAT_DELAY_MS = 300L

/** 连发间隔（毫秒）。400 + 50 的话，按住一秒能重复十来次。 */
private const val REPEAT_INTERVAL_MS = 50L

/**
 * 贴自绘位图的视图：上面一张候选条、下面一张键盘。
 *
 * 两块面都由 `qingjian-render` 出位图，这里只负责画出来、把触摸原样转出去——
 * 不画任何控件、不做任何排版，也**不判断点到了什么**（命中测试在 Rust 里）。
 *
 * 触摸坐标**不分块**：视图的 y 轴就是整块输入视图的 y 轴，候选条在上、键盘在下，
 * 按 y 分派是 Rust 那边的事。所以这里绝不能用两个子视图去拼。
 *
 * 高度由两张位图加起来得到，不自己去算：安卓按视图量出来的尺寸给输入法窗口大小，
 * 报小了会被裁掉，报大了窗口底下留一条空白。
 */
class QingjianSurfaceView(context: Context) : View(context) {
    /** 上方候选条。 */
    private var bar: Bitmap? = null

    /** 下方键盘。 */
    private var keyboard: Bitmap? = null

    /** 视图该有多高（像素），0 表示位图还没到。 */
    private var contentHeight = 0

    /** 屏幕底部被系统手势条 / 导航栏占掉的高度（像素）。 */
    private var bottomInset = 0

    /** 底部被系统占掉多少（点），告诉 Rust 让键往上让开。 */
    val bottomInsetPoints: Float
        get() = bottomInset / resources.displayMetrics.density

    /** 触摸回调 `(actionMasked, pointerId, x, y)`，坐标是整块输入视图的、且是**那根手指**的。 */
    var onTouch: ((Int, Int, Float, Float) -> Unit)? = null

    /**
     * 连发回调：某根手指按住够久了，问一次「要不要再来一下」，参数是那根手指的 pointer id。
     *
     * 这里只负责**计时**，不判断该不该连发——那是输入语义，在 Rust 那边
     * （`action::repeats`）。计时器放在这里是因为安卓有现成的 `Handler`，
     * 为这个给 Rust 引线程或定时器不划算。
     */
    var onRepeat: ((Int) -> Unit)? = null

    /**
     * 移光标的一拍：空格键上按着不放时，每一拍问一次「这一拍走几格」。
     *
     * 走几格、什么时候算「在移光标」都由 Rust 定——壳只管按节拍敲。
     */
    var onCursorTick: ((Int) -> Unit)? = null

    /** 尺寸变化时回调，用来让服务重新告诉 Rust 该画多宽（转屏等）。 */
    var onConfigure: (() -> Unit)? = null

    /** 每根手指按下的时刻，用来判够不够久。 */
    private val downAt = HashMap<Int, Long>()

    /** 已经报过「按住够久了」的手指。每个按下只报一次，免得每 50ms 刷一行日志。 */
    private val reported = HashSet<Int>()

    /**
     * 连发 / 移光标共用的一拍：每 50ms 把所有按着的手指各报一次，再排下一拍。
     *
     * 两者要的时机不一样，所以分了两个回调：**长按连发**要按够 [`REPEAT_DELAY_MS`] 才算，
     * **移光标**是拖动当中就走（等 300ms 才动就太迟了）。
     */
    private val ticker = object : Runnable {
        override fun run() {
            val now = SystemClock.uptimeMillis()
            for ((pointer, at) in downAt) {
                if (now - at >= REPEAT_DELAY_MS) {
                    // 每个按下报一次。**这是「心跳真跳起来了」的唯一证据**——长按连删、
                    // 空格拖光标、长按删词三条路都挂在这个心跳上，而它们在模拟器上都验过、
                    // 真机上出过「按了没反应」。有这行才分得清是心跳没跳，还是心跳跳了但后面没反应。
                    if (reported.add(pointer)) Log.i(TAG, "按住够久了，开始走长按（pointer=$pointer）")
                    onRepeat?.invoke(pointer)
                }
                onCursorTick?.invoke(pointer)
            }
            // 手指还按着就接着排；全松了的话 UP 那边已经把回调撤了
            if (downAt.isNotEmpty()) postDelayed(this, REPEAT_INTERVAL_MS)
        }
    }

    /** 贴候选条。传 `null` 表示这一条现在不该在（没在组句），高度也跟着让出去。 */
    fun setBar(value: Bitmap?) {
        bar = value
        refreshHeight()
        invalidate()
    }

    fun setKeyboard(value: Bitmap) {
        keyboard = value
        refreshHeight()
        invalidate()
    }

    /** 两张位图加起来就是视图该有的高度，变了就重新量一次。 */
    private fun refreshHeight() {
        val pixels = (bar?.height ?: 0) + (keyboard?.height ?: 0)
        if (pixels > 0 && pixels != contentHeight) {
            contentHeight = pixels
            requestLayout()
        }
    }

    override fun onMeasure(widthMeasureSpec: Int, heightMeasureSpec: Int) {
        val width = MeasureSpec.getSize(widthMeasureSpec)
        setMeasuredDimension(width, contentHeight.coerceAtLeast(suggestedMinimumHeight))
    }

    override fun onSizeChanged(w: Int, h: Int, oldw: Int, oldh: Int) {
        super.onSizeChanged(w, h, oldw, oldh)
        onConfigure?.invoke()
    }

    /** 手势条 / 导航栏占掉的高度变了就重配一次。 */
    override fun onApplyWindowInsets(insets: WindowInsets): WindowInsets {
        val bottom = insets.systemGestureInsets.bottom
        if (bottom != bottomInset) {
            bottomInset = bottom
            onConfigure?.invoke()
        }
        return super.onApplyWindowInsets(insets)
    }

    override fun onTouchEvent(event: MotionEvent): Boolean {
        // 按**那根手指**报，不是按 event.x：`event.x` 永远取第 0 根手指的坐标，
        // 而 POINTER_DOWN / POINTER_UP 指的是另一根。两只拇指快速交替时接触时间会重叠，
        // 混着报会让两根手指互相吃掉对方（真机上「点快了掉字母」）。
        val index = event.actionIndex
        val action = event.actionMasked
        val pointer = event.getPointerId(index)
        val y = event.getY(index)

        when (action) {
            MotionEvent.ACTION_DOWN, MotionEvent.ACTION_POINTER_DOWN -> {
                // 按下就震一下，与原生那条路同一个手感。**候选条不震**：那是点选项，不是敲键。
                if (y >= (bar?.height ?: 0)) {
                    keyFeedback(this)
                }
                // 第一根手指落下时才起计时器，后面几根跟着一起算
                if (downAt.isEmpty()) postDelayed(ticker, REPEAT_DELAY_MS)
                downAt[pointer] = SystemClock.uptimeMillis()
                reported.remove(pointer)
            }
            MotionEvent.ACTION_UP, MotionEvent.ACTION_POINTER_UP -> {
                downAt.remove(pointer)
                reported.remove(pointer)
                if (downAt.isEmpty()) removeCallbacks(ticker)
            }
            MotionEvent.ACTION_CANCEL -> {
                downAt.clear()
                reported.clear()
                removeCallbacks(ticker)
            }
        }

        onTouch?.invoke(action, pointer, event.getX(index), y)
        if (event.actionMasked == MotionEvent.ACTION_UP) {
            performClick()
        }
        return true
    }

    override fun performClick(): Boolean {
        super.performClick()
        return true
    }

    override fun onDraw(canvas: Canvas) {
        super.onDraw(canvas)
        // 位图密度是 DENSITY_NONE，这里按 1:1 贴，不会被缩放
        var y = 0f
        bar?.let {
            canvas.drawBitmap(it, 0f, y, null)
            y += it.height
        }
        keyboard?.let { canvas.drawBitmap(it, 0f, y, null) }
    }

    private companion object {
        /** 与 Rust 侧 logcat 标签、[QingjianImeService] 一致：`adb logcat -s Qingjian` 一网打尽。 */
        const val TAG = "Qingjian"
    }
}
