package app.qingjian.android

import android.content.Context
import android.graphics.Bitmap
import android.graphics.Canvas
import android.view.MotionEvent
import android.view.View
import android.view.WindowInsets

/**
 * 贴一张自绘位图的视图。
 *
 * 候选条与键盘都由 `qingjian-render` 出位图，这里只负责画出来、把触摸原样转出去——
 * 不画任何控件、不做任何排版，也**不判断点到了什么**（命中测试在 Rust 里）。
 *
 * 高度由 Rust 告知（键盘主题定的），自己不去猜：安卓按视图量出来的尺寸给输入法窗口大小，
 * 不给高度的话窗口会被撑满整屏。
 */
class QingjianSurfaceView(context: Context) : View(context) {
    private var bitmap: Bitmap? = null

    /** 视图该有多高（像素），0 表示还不知道。 */
    private var contentHeight = 0

    /** 屏幕底部被系统手势条 / 导航栏占掉的高度（像素）。 */
    private var bottomInset = 0

    /** 底部被系统占掉多少（点），告诉 Rust 让键往上让开。 */
    val bottomInsetPoints: Float
        get() = bottomInset / resources.displayMetrics.density

    /** 触摸回调 `(actionMasked, x, y)`。 */
    var onTouch: ((Int, Float, Float) -> Unit)? = null

    /** 尺寸变化时回调，用来让服务重新告诉 Rust 该画多宽（转屏等）。 */
    var onConfigure: (() -> Unit)? = null

    fun setBitmap(value: Bitmap) {
        bitmap = value
        invalidate()
    }

    /** Rust 告知键盘该有多高（点），变了就重新量一次。 */
    fun setContentHeightPoints(points: Float) {
        val pixels = (points * resources.displayMetrics.density).toInt()
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
        bitmap?.let { canvas.drawBitmap(it, 0f, 0f, null) }
    }
}
