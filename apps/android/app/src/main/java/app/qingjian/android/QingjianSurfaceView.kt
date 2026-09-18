package app.qingjian.android

import android.content.Context
import android.graphics.Bitmap
import android.graphics.Canvas
import android.view.MotionEvent
import android.view.View
import android.view.WindowInsets

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

    /** 触摸回调 `(actionMasked, x, y)`，坐标是整块输入视图的。 */
    var onTouch: ((Int, Float, Float) -> Unit)? = null

    /** 尺寸变化时回调，用来让服务重新告诉 Rust 该画多宽（转屏等）。 */
    var onConfigure: (() -> Unit)? = null

    fun setBar(value: Bitmap) {
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
        onTouch?.invoke(event.actionMasked, event.x, event.y)
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
}
