// 依赖从哪下：国内镜像排前面，官方源留后面兜底。
// Maven 中央仓库在国内经常握不上手（Remote host terminated the handshake），
// 走阿里云镜像稳定得多；换成别的网络环境时删掉那三行即可。
pluginManagement {
    repositories {
        maven("https://maven.aliyun.com/repository/public")
        maven("https://maven.aliyun.com/repository/google")
        maven("https://maven.aliyun.com/repository/gradle-plugin")
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}

dependencyResolutionManagement {
    repositories {
        maven("https://maven.aliyun.com/repository/public")
        maven("https://maven.aliyun.com/repository/google")
        google()
        mavenCentral()
    }
}

rootProject.name = "qingjian"
include(":app")
