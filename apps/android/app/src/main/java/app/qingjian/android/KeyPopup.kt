package app.qingjian.android

import android.content.Context
import android.graphics.Bitmap
import android.view.Gravity
import android.view.View
import android.widget.ImageView
import android.widget.PopupWindow

/**
 * 键预览气泡的浮动小窗。
 *
 * 气泡本身是 Rust 画好的一张位图（与候选条、键盘同一条路），这里只是个**承载**：
 * 把图贴上去、挪到 Rust 算好的位置。**不画任何东西**，也不做布局计算。
 *
 * 为什么要单开一个窗、而不是在输入视图里腾一块地方：气泡要弹到键盘**上方**去，
 * 而输入法窗口只有输入视图那么高，画在里面会被窗口裁掉。腾地方的另一条路是
 * 常驻一条透明带，但那一块**既占屏幕又吃触摸**（点在键盘正上方会没反应），不要。
 */
class KeyPopup(context: Context) {
    private val image = ImageView(context)

    private val window = PopupWindow(image, 0, 0).apply {
        // 气泡比键大一圈，会盖到键盘上——不许裁
        isClippingEnabled = false
        isFocusable = false
        // **不吃触摸**：气泡底下的键还按得着，手指该落在键盘上
        isTouchable = false
        // 系统背景不要，位图自己带圆角与阴影
        setBackgroundDrawable(null)
        inputMethodMode = PopupWindow.INPUT_METHOD_NOT_NEEDED
    }

    /**
     * 贴一张新位图并挪到 `(x, y)`。`bitmap` 为 `null` 表示收起来。
     *
     * `(x, y)` 是**整块输入视图**的像素坐标（Rust 算好的位图左上角），
     * **不必换算成屏幕坐标**：这个窗是输入法窗口的子窗（`mParentWindow=InputMethod`），
     * 坐标本来就相对父窗——而输入视图正好铺满父窗。
     *
     * 踩过：按屏幕坐标传（自己加 `getLocationOnScreen`），整块跑到屏幕外面去了
     * （`frame=[505,1254][703,1489]`，屏幕才 1280 高）——父窗的偏移被算了两遍。
     * 气泡`y` 是**负的**（在键盘上方），靠 `isClippingEnabled = false` 才画得出来。
     */
    fun show(anchor: View, bitmap: Bitmap?, x: Float, y: Float) {
        if (bitmap == null) {
            dismiss()
            return
        }
        image.setImageBitmap(bitmap)
        val left = x.toInt()
        val top = y.toInt()

        if (window.isShowing) {
            window.update(left, top, bitmap.width, bitmap.height)
        } else {
            window.width = bitmap.width
            window.height = bitmap.height
            // 用 TOP|START 而不是 NO_GRAVITY：那一个在不同版本上偏移量的解释不一样
            window.showAtLocation(anchor, Gravity.TOP or Gravity.START, left, top)
        }
    }

    fun dismiss() {
        if (window.isShowing) window.dismiss()
    }
}
