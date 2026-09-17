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
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    // .so 由 cargo ndk 编好放在 src/main/jniLibs/，Gradle 直接打进去，不再接构建任务。
    sourceSets["main"].jniLibs.srcDir("src/main/jniLibs")
}
