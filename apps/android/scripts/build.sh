#!/usr/bin/env bash
# 把 qingjian-android 编成 APK，可以顺手装到设备上。
#
#   scripts/build.sh                  # 编 .so（两个 ABI）+ 打词库 + 打 debug APK
#   scripts/build.sh --abi x86_64     # 只编一个 ABI（模拟器是 x86_64，快得多）
#   scripts/build.sh --install        # 打完装到已连接的设备并启用输入法
#   scripts/build.sh --skip-rust      # 跳过 cargo（只改了 Kotlin 时用）
#
# 环境变量（都能不设，脚本会自己找）：
#   ANDROID_HOME       Android SDK。缺省读环境变量，再依次找 D:\DSH\android-sdk / ~/Android/Sdk / ~/Library/Android/sdk
#   ANDROID_NDK_HOME   NDK。缺省取 $ANDROID_HOME/ndk 下版本号最大的那个
#   QINGJIAN_GRADLE    gradle 可执行文件。缺省找 PATH 里的 gradle，再找 D:\DSH\gradle-*，最后找仓库里的 gradlew
#   JAVA_HOME          gradle 要 Java 17+。缺省找 C:\Program Files\Java\jdk-17
#
# 词库：data/generated/dict.qj 优先；没有就用 data/generated/dict.tsv 现打；再退回基础词库
# assets/lexicon/dict.tsv（9.3 万条，够翻页）；最后才是随包的样例（148 条，只够验通路）。
# 打出来的 .qj 落在 target/android/（不动仓库里的文件），再拷一份进 APK 的 assets——
# 词库跟着包走，装完就能用，不需要 adb 往应用私有目录里推。
#
# APK 是自包含的：词库、emoji 字体、emoji 表都在 assets 里，首次唤起输入法时自己解到私有目录。
#
# .so 编完会 llvm-strip 掉调试信息（38 MB → 8 MB），为的是别让 jniLibs 白占 60 MB、gradle 打包也快些。
# 用 `--strip-debug` 而不是 `--strip-all`，留着符号表，Rust panic 的调用栈还能看。
# 注意这一步只影响 jniLibs 占的磁盘和打包耗时，**不影响 APK 大小**——AGP 打包时自己还会 strip 一遍。
# 实测（debug、两个 ABI）：APK 44 MB，进包的 .so 是 13.7 MB（arm64-v8a）+ 16.0 MB（x86_64），未压缩存贮。
#
# --install 会自动 `ime enable` + `ime set` 切到青简，不用手点系统设置。
#
# 用 --release：调试包快得多。debug 的 Rust 没优化，键盘一次渲染要 19.7ms，release 只要 1.26ms
# （桌面 CPU 上量的），而每敲一下要重画两次键盘——真机上打字跟不跟得上手感就看这个。
# release 目前用调试密钥签（见 app/build.gradle.kts），能装能测，但不能拿去分发。
set -euo pipefail

HERE="$(cd "$(dirname "$0")/.." && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
PKG="app.qingjian.android"
IME="$PKG/.QingjianImeService"

PROFILE=debug
INSTALL=0
SKIP_RUST=0
STRIP=1
ABIS=()

# Git Bash 里环境变量可能是 Windows 写法（D:\...），转成 POSIX 给 cargo / gradle 用
to_posix() {
  if command -v cygpath >/dev/null 2>&1; then cygpath -u "$1"; else printf '%s' "$1"; fi
}

# 反过来：adb 是 Windows 程序，本机路径要给它 Windows 写法。非 Git Bash 环境原样返回。
winpath() {
  if command -v cygpath >/dev/null 2>&1; then cygpath -w "$1"; else printf '%s' "$1"; fi
}

usage() {
  # 印开头的注释块（跳过 shebang），遇到第一行非注释就停
  awk 'NR == 1 { next } /^#/ { sub(/^# ?/, ""); print; next } { exit }' "$0"
  exit 0
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --release) PROFILE=release ;;
    --debug) PROFILE=debug ;;
    --install) INSTALL=1 ;;
    --skip-rust) SKIP_RUST=1 ;;
    --no-strip) STRIP=0 ;;
    --abi) shift; ABIS+=("$1") ;;
    -h|--help) usage ;;
    *) echo "不认识的参数: $1（--help 看用法）" >&2; exit 1 ;;
  esac
  shift
done
[[ ${#ABIS[@]} -eq 0 ]] && ABIS=(x86_64 arm64-v8a)

# SDK 与 NDK
if [[ -n "${ANDROID_HOME:-}" ]]; then ANDROID_HOME="$(to_posix "$ANDROID_HOME")"; fi
if [[ -z "${ANDROID_HOME:-}" || ! -d "$ANDROID_HOME" ]]; then
  for candidate in "/d/DSH/android-sdk" "$HOME/Android/Sdk" "$HOME/Library/Android/sdk"; do
    if [[ -d "$candidate" ]]; then ANDROID_HOME="$candidate"; break; fi
  done
fi
[[ -d "${ANDROID_HOME:-}" ]] || { echo "找不到 Android SDK，设一下 ANDROID_HOME" >&2; exit 1; }
export ANDROID_HOME ANDROID_SDK_ROOT="$ANDROID_HOME"

if [[ -n "${ANDROID_NDK_HOME:-}" ]]; then ANDROID_NDK_HOME="$(to_posix "$ANDROID_NDK_HOME")"; fi
if [[ -z "${ANDROID_NDK_HOME:-}" || ! -d "$ANDROID_NDK_HOME" ]]; then
  ANDROID_NDK_HOME="$(find "$ANDROID_HOME/ndk" -maxdepth 1 -mindepth 1 -type d 2>/dev/null | sort | tail -1)"
fi
[[ -d "${ANDROID_NDK_HOME:-}" ]] || { echo "找不到 NDK，设一下 ANDROID_NDK_HOME" >&2; exit 1; }
export ANDROID_NDK_HOME

# gradle 要 Java 17+
if [[ -z "${JAVA_HOME:-}" ]]; then
  for candidate in "/c/Program Files/Java/jdk-17" "$HOME/Android/jdk" ; do
    if [[ -x "$candidate/bin/java" || -x "$candidate/bin/java.exe" ]]; then JAVA_HOME="$candidate"; break; fi
  done
fi
[[ -n "${JAVA_HOME:-}" ]] || { echo "找不到 JDK，设一下 JAVA_HOME" >&2; exit 1; }
export JAVA_HOME

# gradle
GRADLE="${QINGJIAN_GRADLE:-}"
if [[ -z "$GRADLE" ]]; then
  if command -v gradle >/dev/null 2>&1; then
    GRADLE="$(command -v gradle)"
  elif [[ -x "$HERE/gradlew" ]]; then
    GRADLE="$HERE/gradlew"
  else
    GRADLE="$(ls -d /d/DSH/gradle-*/bin/gradle 2>/dev/null | sort | tail -1 || true)"
  fi
fi
[[ -n "$GRADLE" && -x "$GRADLE" ]] || { echo "找不到 gradle，设一下 QINGJIAN_GRADLE" >&2; exit 1; }

echo "SDK    : $ANDROID_HOME"
echo "NDK    : $ANDROID_NDK_HOME"
echo "gradle : $GRADLE"
echo "JAVA   : $JAVA_HOME"
echo "ABI    : ${ABIS[*]}   配置: $PROFILE"

cd "$ROOT"

# 编引擎 .so 到 jniLibs（cargo-ndk 按 ABI 建子目录，与 build.gradle.kts 的 jniLibs.srcDir 对得上）
if [[ "$SKIP_RUST" -eq 0 ]]; then
  ndk_targets=()
  for abi in "${ABIS[@]}"; do ndk_targets+=(-t "$abi"); done
  cargo_args=(-p qingjian-android --locked)
  [[ "$PROFILE" == "release" ]] && cargo_args+=(--release)
  echo "== 编引擎 .so（${ABIS[*]}）=="
  cargo ndk --platform 29 "${ndk_targets[@]}" -o "$HERE/app/src/main/jniLibs" build "${cargo_args[@]}"

  if [[ "$STRIP" -eq 1 ]]; then
    stripper="$(ls "$ANDROID_NDK_HOME"/toolchains/llvm/prebuilt/*/bin/llvm-strip* 2>/dev/null | head -1)"
    if [[ -n "$stripper" ]]; then
      echo "== strip 调试信息 =="
      for so in "$HERE"/app/src/main/jniLibs/*/*.so; do
        [[ -f "$so" ]] || continue
        before="$(du -m "$so" | cut -f1)"
        "$stripper" --strip-debug "$so"
        echo "  $(basename "$(dirname "$so")")/$(basename "$so")  ${before} MB → $(du -m "$so" | cut -f1) MB"
      done
    else
      echo "  没找到 llvm-strip，跳过（APK 会大不少）" >&2
    fi
  fi
fi

# 打词库
DICT_OUT="$ROOT/target/android/dict.qj"
mkdir -p "$(dirname "$DICT_OUT")"
pack_dict() {
  echo "== 打词库（$1）=="
  cargo run --release -q -p qingjian-dict-convert -- \
    --out-dir "$ROOT/target/android" pack dict \
    --input "$1" --name "$2" --license "GPL-3.0-or-later"
}
if [[ -f "$ROOT/data/generated/dict.qj" ]]; then
  echo "== 用现成的 data/generated/dict.qj =="
  cp "$ROOT/data/generated/dict.qj" "$DICT_OUT"
elif [[ -f "$ROOT/data/generated/dict.tsv" ]]; then
  pack_dict "$ROOT/data/generated/dict.tsv" "青简词库"
elif [[ -f "$ROOT/assets/lexicon/dict.tsv" ]]; then
  pack_dict "$ROOT/assets/lexicon/dict.tsv" "青简基础词库"
elif [[ -f "$ROOT/assets/sample/dict.tsv" ]]; then
  pack_dict "$ROOT/assets/sample/dict.tsv" "青简样例词库"
else
  echo "找不到词库（data/generated/、assets/lexicon/ 与 assets/sample/ 都没有）" >&2
  exit 1
fi

# 词库打进 APK 的 assets，跟着包走。放在这里而不是 assets/emoji 那种「源就在仓库里」的位置，
# 是因为词库是现打的产品数据，仓库里没有、也不该有。
DICT_ASSET="$HERE/app/src/main/assets/dict.qj"
mkdir -p "$(dirname "$DICT_ASSET")"
cp "$DICT_OUT" "$DICT_ASSET"

# 英文词表：同样跟着包走（取了就放进随包资源目录，英文模式才有补全与拼错纠正）。
# 优先顺序与 mac 的 bundle.sh 一致：仓库里那份在前，生成出来的在后。
# **没有它不算错**——英文模式会退回直输，只是少了候选，见 apps/android/src/session/mod.rs。
ENGLISH_ASSET="$HERE/app/src/main/assets/english.tsv"
ENGLISH_SRC=""
for candidate in "$ROOT/assets/lexicon/english.tsv" "$ROOT/data/generated/english.tsv"; do
  if [[ -f "$candidate" ]]; then ENGLISH_SRC="$candidate"; break; fi
done
if [[ -n "$ENGLISH_SRC" ]]; then
  echo "== 英文词表（$ENGLISH_SRC）=="
  cp "$ENGLISH_SRC" "$ENGLISH_ASSET"
else
  echo "== 找不到英文词表，英文模式将退回直输 ==" >&2
  rm -f "$ENGLISH_ASSET"
fi

# 打 APK
echo "== 打 APK（$PROFILE）=="
( cd "$HERE" && "$GRADLE" --quiet "assemble${PROFILE^}" )

if [[ "$PROFILE" == "debug" ]]; then
  APK="$HERE/app/build/outputs/apk/debug/app-debug.apk"
else
  APK="$HERE/app/build/outputs/apk/release/app-release.apk"
fi
[[ -f "$APK" ]] || { echo "没找到打出来的 APK：$APK" >&2; exit 1; }
echo "  $APK  ($(du -m "$APK" | cut -f1) MB)"

# 装到设备
if [[ "$INSTALL" -eq 1 ]]; then
  ADB="$ANDROID_HOME/platform-tools/adb"
  echo "== 装到设备 =="
  "$ADB" install -r "$(winpath "$APK")"
  "$ADB" shell ime enable "$IME"
  "$ADB" shell ime set "$IME"
  echo "  输入法就绪，键盘里已经切到青简"
  echo "  （词库与 emoji 字体都在 APK 里，首次唤起时自己解到应用私有目录，不用 adb 推）"
fi
