plugins {
    // AGP 9 起自带 Kotlin 支持，不再需要单独的 kotlin.android 插件。
    id("com.android.application")
}

android {
    namespace = "app.qingjian.android"
    compileSdk = 36
    // 渲染器取系统字体走 ASystemFontIterator，它从 API 29 起才有；
    // 定 29 就不用给更老的机器留一条回退路径。
    ndkVersion = "28.2.13676358"

    defaultConfig {
        applicationId = "app.qingjian.android"
        minSdk = 29
        targetSdk = 36
        versionCode = 1
        versionName = "0.1.0-dev"

        ndk {
            // 模拟器是 x86_64，真机是 arm64；两个都带上，装哪台都能跑。
            abiFilters += listOf("x86_64", "arm64-v8a")
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            // 正式的签名密钥还没有，先拿调试密钥签：能装、能测，但**不能拿去分发**。
            // 要发分发版得自己生成 keystore 换掉这一行。
            //
            // 为什么需要它：release 构建的 Rust 是开了优化的，键盘一次渲染 debug 要 19.7ms、
            // release 只要 1.26ms（桌面 CPU 上量过），打字手感完全两回事。
            signingConfig = signingConfigs.getByName("debug")
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    // .so 由 cargo ndk 编好放在 src/main/jniLibs/，Gradle 直接打进去，不再接构建任务。
    sourceSets["main"].jniLibs.srcDir("src/main/jniLibs")

    // emoji 字体与 emoji 表放在仓库的 assets/emoji/（来源与许可见那里的 README），整个目录挂进来跟着 APK 走。
    // 系统自带的那张 emoji 字体在安卓 15 起换成了渲染器画不出的格式，所以随包带一张。
    sourceSets["main"].assets.srcDir("../../../assets/emoji")
    // 颜文字面板那张表（来源与许可见 assets/kaomoji/README.md）。两个目录是**平铺**进 assets 根的
    // （srcDir 不保留目录名），所以表名带前缀区分：emoji-panel.tsv / kaomoji-panel.tsv。
    sourceSets["main"].assets.srcDir("../../../assets/kaomoji")
}
