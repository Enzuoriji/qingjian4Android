package app.qingjian.android

import android.content.Context
import android.os.Build
import android.os.VibrationEffect
import android.os.Vibrator
import android.os.VibratorManager
import android.view.View

/**
 * 敲一下键的震动反馈。**两条路共用**——震动是键盘本来就该有的手感，不是哪条路的优点。
 *
 * **直连马达，不走 `View.performHapticFeedback`。** 那条路要经过「视图 → 窗口 → 系统」三层转手，
 * 其中任何一层不买账都是**静默不震**，还分不出是哪一层：真机上装了带
 * `VIRTUAL_KEY` + `FLAG_IGNORE_GLOBAL_SETTING` + `VIBRATE` 权限的包，两种键盘都不震，
 * 而同一台机器上 Gboard 震得好好的——说明马达与系统那层没问题，是这条转手路不通。
 * 直连马达只有一步，成不成一眼看得出来。
 *
 * 时长是**试出来的起点，不是定稿**：真机上要按手感调。各家输入法在这个基础上还有
 * 「长按与抬起力度不同」之类的花样（fcitx5-android 就有），那要等有设置项了再说。
 */
fun keyFeedback(view: View) {
    val motor = motor(view.context) ?: return
    motor.vibrate(VibrationEffect.createOneShot(DURATION_MS, VibrationEffect.DEFAULT_AMPLITUDE))
}

/** 敲一下震多久（毫秒）。 */
private const val DURATION_MS = 20L

/** 取马达。取一次记下来：每敲一下都去问一次系统服务是白花时间。 */
private var cached: Vibrator? = null

private fun motor(context: Context): Vibrator? {
    cached?.let { return it.takeIf(Vibrator::hasVibrator) }
    val manager = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
        context.getSystemService(VibratorManager::class.java)?.defaultVibrator
    } else {
        @Suppress("DEPRECATION")
        context.getSystemService(Vibrator::class.java)
    }
    cached = manager
    return manager?.takeIf(Vibrator::hasVibrator)
}
