//! 交给 Kotlin 的一张位图：8 字节头（宽、高，各 u32 大端）+ 预乘 RGBA 像素。
//!
//! Kotlin 侧 `Bitmap.createBitmap(w, h, ARGB_8888)` + `copyPixelsFromBuffer` 直接吃这段字节：
//! `ARGB_8888` 的内存布局就是预乘 RGBA（`ARGB` 只是 `getPixel` 那套打包的说法），与 tiny-skia 的
//! `Pixmap` 一致，既不用换通道也不用重新预乘。**这条「不用转换」当初用一次性探针在本机与设备上实测确认过**
//! （探针已随收尾删掉，结论留着），
//! macOS 壳那边也是同样的结论（`NSBitmapImageRep` 逐行拷贝、零转换）。

use qingjian_render::Pixmap;

/// 头部字节数：宽、高各 4 字节。
pub(crate) const HEADER_LEN: usize = 8;

/// 一张位图编成能过 JNI 的字节串。
pub(crate) fn encode(pixmap: &Pixmap) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(HEADER_LEN + pixmap.data().len());
    bytes.extend_from_slice(&pixmap.width().to_be_bytes());
    bytes.extend_from_slice(&pixmap.height().to_be_bytes());
    bytes.extend_from_slice(pixmap.data());
    bytes
}
