package app.qingjian.android

import android.content.Context
import android.os.Build
import android.os.VibrationEffect
import android.os.Vibrator
import android.os.VibratorManager
import android.view.View
import org.json.JSONObject

/**
 * 敲一下键的震动反馈。**两条路共用**——震动是键盘本来就该有的手感，不是哪条路的优点。
 *
 * **直连马达，不走 `View.performHapticFeedback`。** 那条路要经过「视图 → 窗口 → 系统」三层转手，
 * 其中任何一层不买账都是**静默不震**，还分不出是哪一层：真机上装了带
 * `VIRTUAL_KEY` + `FLAG_IGNORE_GLOBAL_SETTING` + `VIBRATE` 权限的包，两种键盘都不震，
 * 而同一台机器上 Gboard 震得好好的——说明马达与系统那层没问题，是这条转手路不通。
 * 直连马达只有一步，成不成一眼看得出来。
 *
 * 震什么感觉由 [`style`] 定（[设置页](QingjianSettingsActivity) 上那几档）。除了自己定时长那一档，
 * 都交给系统的**预置触感**：厂商针对自家马达调过，而一个手填的毫秒数在每台机器上表现都不一样。
 * 设备不支持某个预置效果时系统会退回平台波形，所以不必去查 `areEffectsSupported`。
 */
fun keyFeedback(view: View) {
    val current = vibrationStyle
    if (current == VibrationStyle.Off) return
    val motor = motor(view.context) ?: return
    val effect = current.effect?.let { VibrationEffect.createPredefined(it) }
        ?: VibrationEffect.createOneShot(vibrationMs, VibrationEffect.DEFAULT_AMPLITUDE)
    motor.vibrate(effect)
}

/**
 * 按键震动的感觉。
 *
 * **键名与 `crates/qingjian-platform/src/config/keyboard.rs` 的 `VibrationStyle` 一一对应**
 * （也就是配置文件里 `[keyboard] vibration` 那几个取值），改一边必须同时改另一边——
 * 跨语言没得共享。四档的强弱排序也是那边定的：轻 < 清脆 < 低沉 < 双击。
 */
enum class VibrationStyle(val key: String, val effect: Int?) {
    /** 不震。 */
    Off("off", null),

    /** 轻微。 */
    Tick("tick", VibrationEffect.EFFECT_TICK),

    /** 清脆。缺省：官方建议的基准档，介于轻与重之间。 */
    Click("click", VibrationEffect.EFFECT_CLICK),

    /** 低沉。 */
    Heavy("heavy", VibrationEffect.EFFECT_HEAVY_CLICK),

    /** 双击。 */
    Double("double", VibrationEffect.EFFECT_DOUBLE_CLICK),

    /** 自定义时长，见 [vibrationMs]。 */
    Custom("custom", null),
    ;

    companion object {
        /** 按配置里的写法取一档；认不出来按 [Click]。 */
        fun of(key: String): VibrationStyle = entries.firstOrNull { it.key == key } ?: Click
    }
}

/** 当前该怎么震。**壳在键盘弹出来时按配置设一次**（见 [applyVibrationConfig]），不是每敲一下读文件。 */
private var vibrationStyle = VibrationStyle.Click

/** 自定义档震多久（毫秒）。与 `qingjian_platform` 的 `DEFAULT_VIBRATION_MS` 一致。 */
private var vibrationMs = DEFAULT_VIBRATION_MS

/** 自定义档的缺省时长（毫秒）。 */
private const val DEFAULT_VIBRATION_MS = 20L

/**
 * 从配置里读震动那两项。[QingjianImeService.onStartInputView] 每次键盘弹出来时调一遍
 * （键盘弹一次读一次文件，跟每敲一下读一次是两回事）。
 *
 * 读不出来就保持当前值——震动是手感，配置坏了不该把它一起弄没。
 */
fun applyVibrationConfig(envelope: String?) {
    val keyboard = envelope
        ?.let { runCatching { JSONObject(it) }.getOrNull() }
        ?.takeIf { it.optBoolean("ok") }
        ?.optJSONObject("config")
        ?.optJSONObject("keyboard")
        ?: return
    vibrationStyle = VibrationStyle.of(keyboard.optString("vibration", VibrationStyle.Click.key))
    vibrationMs = keyboard.optInt("vibration_ms", DEFAULT_VIBRATION_MS.toInt()).toLong()
}

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
