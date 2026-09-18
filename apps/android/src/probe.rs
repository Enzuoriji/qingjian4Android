//! 位图通路的探针（M0 临时件，键盘接上之后删）。
//!
//! 只为证明一件事：`Pixmap` → JNI → 安卓 `Bitmap` 这条路上一个像素都没错。
//! 分两张，出错时能立刻分出是哪种错：
//!
//! - **色块**（不依赖字体）：红 / 绿 / 蓝 / 半透明白四块。红蓝互换 = 字节序错；
//!   第四块画出来是亮白而不是中灰 = 预乘理解错。
//! - **文字**（依赖字体）：一行中日英混排加 emoji。出豆腐块 = 字体清单或回退表没命中。

use qingjian_render::{Frame, Layout, Pixmap, RenderError, Renderer, Row, Theme};

/// 每个色块的边长（像素）。四个等宽摆一排，Kotlin 侧按位图宽度就能算出各块中心。
const BLOCK: u32 = 64;

/// 四个色块，从左到右，值是**预乘 RGBA**。
///
/// 第四个是 50% 透明白：预乘后仍是 `(128,128,128,128)`，画在黑底上应是中灰。
/// 要是被当成非预乘数据解读，它会变成亮白——这就是预乘那一环的探针。
const BLOCKS: [[u8; 4]; 4] = [
    [255, 0, 0, 255],
    [0, 255, 0, 255],
    [0, 0, 255, 255],
    [128, 128, 128, 128],
];

/// 画四个色块。
pub(crate) fn colors() -> Pixmap {
    let width = BLOCK * BLOCKS.len() as u32;
    let mut pixmap = Pixmap::new(width, BLOCK).expect("探针位图建不起来");
    for (i, color) in BLOCKS.iter().enumerate() {
        let x0 = i as u32 * BLOCK;
        for y in 0..BLOCK {
            for x in x0..x0 + BLOCK {
                let at = ((y * width + x) * 4) as usize;
                pixmap.data_mut()[at..at + 4].copy_from_slice(color);
            }
        }
    }
    pixmap
}

/// 文字探针画的那一行。
///
/// 挑的都是有讲究的字：「青简」验中文、`日本語` 验假名、「骨直曜」是中日同形字的经典试金石
/// （两边字形不同，落错面一眼能看出来）、🙂 验彩色 emoji。
pub(crate) const SAMPLE: &str = "青简 hello 🙂 日本語 骨直曜";

/// 画 [`SAMPLE`]，走真正的渲染路径，验字体清单与回退表。
pub(crate) fn text(renderer: &mut Renderer) -> Result<Pixmap, RenderError> {
    let mut frame = Frame::default();
    frame.rows.push(Row::plain(0, SAMPLE));
    let rendered = renderer.render(&frame, Layout::Vertical, &Theme::light(), 2.0, None)?;
    Ok(rendered.pixmap)
}
