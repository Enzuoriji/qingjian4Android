//! 键盘上那几个图标的路径：⇧ 大小写、⌫ 退格、剪贴板（各一个函数 + 一个包围盒）。
//!
//! **这个文件是生成的**，路径数据来自 `assets/icon/material/` 里那几张 svg
//! （Google 的 Material Symbols，Apache-2.0）：
//!
//! ```sh
//! python assets/icon/render-key-icon-path.py
//! ```
//!
//! 换了图标就重跑那条命令，别手改这里。坐标是 Material 那套 960×960、y 轴朝上的网格
//! （所以 y 是负的），缩放与居中在 [`super`] 里做。
#![allow(clippy::approx_constant)]

use tiny_skia::{Path, PathBuilder};

/// 上档（大小写）——Material 的 `keyboard_capslock`。
pub(super) fn shift() -> Option<Path> {
    let mut builder = PathBuilder::new();
    builder.move_to(240.0, -240.0);
    builder.line_to(240.0, -320.0);
    builder.line_to(720.0, -320.0);
    builder.line_to(720.0, -240.0);
    builder.line_to(240.0, -240.0);
    builder.close();
    builder.move_to(480.0, -736.0);
    builder.line_to(720.0, -496.0);
    builder.line_to(664.0, -440.0);
    builder.line_to(480.0, -624.0);
    builder.line_to(296.0, -440.0);
    builder.line_to(240.0, -496.0);
    builder.line_to(480.0, -736.0);
    builder.close();
    builder.finish()
}

/// 退格——Material 的 `backspace`。
pub(super) fn backspace() -> Option<Path> {
    let mut builder = PathBuilder::new();
    builder.move_to(456.0, -320.0);
    builder.line_to(560.0, -424.0);
    builder.line_to(664.0, -320.0);
    builder.line_to(720.0, -376.0);
    builder.line_to(616.0, -480.0);
    builder.line_to(720.0, -584.0);
    builder.line_to(664.0, -640.0);
    builder.line_to(560.0, -536.0);
    builder.line_to(456.0, -640.0);
    builder.line_to(400.0, -584.0);
    builder.line_to(504.0, -480.0);
    builder.line_to(400.0, -376.0);
    builder.line_to(456.0, -320.0);
    builder.close();
    builder.move_to(360.0, -160.0);
    builder.quad_to(341.0, -160.0, 324.0, -168.5);
    builder.quad_to(307.0, -177.0, 296.0, -192.0);
    builder.line_to(80.0, -480.0);
    builder.line_to(296.0, -768.0);
    builder.quad_to(307.0, -783.0, 324.0, -791.5);
    builder.quad_to(341.0, -800.0, 360.0, -800.0);
    builder.line_to(800.0, -800.0);
    builder.quad_to(833.0, -800.0, 856.5, -776.5);
    builder.quad_to(880.0, -753.0, 880.0, -720.0);
    builder.line_to(880.0, -240.0);
    builder.quad_to(880.0, -207.0, 856.5, -183.5);
    builder.quad_to(833.0, -160.0, 800.0, -160.0);
    builder.line_to(360.0, -160.0);
    builder.close();
    builder.move_to(180.0, -480.0);
    builder.line_to(360.0, -240.0);
    builder.line_to(800.0, -240.0);
    builder.line_to(800.0, -720.0);
    builder.line_to(360.0, -720.0);
    builder.line_to(180.0, -480.0);
    builder.close();
    builder.move_to(580.0, -480.0);
    builder.close();
    builder.finish()
}

/// 剪贴板（工具页那一格）——Material 的 `content_paste`。
pub(super) fn clipboard() -> Option<Path> {
    let mut builder = PathBuilder::new();
    builder.move_to(200.0, -120.0);
    builder.quad_to(167.0, -120.0, 143.5, -143.5);
    builder.quad_to(120.0, -167.0, 120.0, -200.0);
    builder.line_to(120.0, -760.0);
    builder.quad_to(120.0, -793.0, 143.5, -816.5);
    builder.quad_to(167.0, -840.0, 200.0, -840.0);
    builder.line_to(367.0, -840.0);
    builder.quad_to(378.0, -875.0, 410.0, -897.5);
    builder.quad_to(442.0, -920.0, 480.0, -920.0);
    builder.quad_to(520.0, -920.0, 551.5, -897.5);
    builder.quad_to(583.0, -875.0, 594.0, -840.0);
    builder.line_to(760.0, -840.0);
    builder.quad_to(793.0, -840.0, 816.5, -816.5);
    builder.quad_to(840.0, -793.0, 840.0, -760.0);
    builder.line_to(840.0, -200.0);
    builder.quad_to(840.0, -167.0, 816.5, -143.5);
    builder.quad_to(793.0, -120.0, 760.0, -120.0);
    builder.line_to(200.0, -120.0);
    builder.close();
    builder.move_to(200.0, -200.0);
    builder.line_to(760.0, -200.0);
    builder.line_to(760.0, -760.0);
    builder.line_to(680.0, -760.0);
    builder.line_to(680.0, -640.0);
    builder.line_to(280.0, -640.0);
    builder.line_to(280.0, -760.0);
    builder.line_to(200.0, -760.0);
    builder.line_to(200.0, -200.0);
    builder.close();
    builder.move_to(508.5, -771.5);
    builder.quad_to(520.0, -783.0, 520.0, -800.0);
    builder.quad_to(520.0, -817.0, 508.5, -828.5);
    builder.quad_to(497.0, -840.0, 480.0, -840.0);
    builder.quad_to(463.0, -840.0, 451.5, -828.5);
    builder.quad_to(440.0, -817.0, 440.0, -800.0);
    builder.quad_to(440.0, -783.0, 451.5, -771.5);
    builder.quad_to(463.0, -760.0, 480.0, -760.0);
    builder.quad_to(497.0, -760.0, 508.5, -771.5);
    builder.close();
    builder.finish()
}

/// 表情（工具页那一格）——Material 的 `mood`。
pub(super) fn mood() -> Option<Path> {
    let mut builder = PathBuilder::new();
    builder.move_to(620.0, -520.0);
    builder.quad_to(645.0, -520.0, 662.5, -537.5);
    builder.quad_to(680.0, -555.0, 680.0, -580.0);
    builder.quad_to(680.0, -605.0, 662.5, -622.5);
    builder.quad_to(645.0, -640.0, 620.0, -640.0);
    builder.quad_to(595.0, -640.0, 577.5, -622.5);
    builder.quad_to(560.0, -605.0, 560.0, -580.0);
    builder.quad_to(560.0, -555.0, 577.5, -537.5);
    builder.quad_to(595.0, -520.0, 620.0, -520.0);
    builder.close();
    builder.move_to(340.0, -520.0);
    builder.quad_to(365.0, -520.0, 382.5, -537.5);
    builder.quad_to(400.0, -555.0, 400.0, -580.0);
    builder.quad_to(400.0, -605.0, 382.5, -622.5);
    builder.quad_to(365.0, -640.0, 340.0, -640.0);
    builder.quad_to(315.0, -640.0, 297.5, -622.5);
    builder.quad_to(280.0, -605.0, 280.0, -580.0);
    builder.quad_to(280.0, -555.0, 297.5, -537.5);
    builder.quad_to(315.0, -520.0, 340.0, -520.0);
    builder.close();
    builder.move_to(603.5, -298.5);
    builder.quad_to(659.0, -337.0, 684.0, -400.0);
    builder.line_to(276.0, -400.0);
    builder.quad_to(301.0, -337.0, 356.5, -298.5);
    builder.quad_to(412.0, -260.0, 480.0, -260.0);
    builder.quad_to(548.0, -260.0, 603.5, -298.5);
    builder.close();
    builder.move_to(324.0, -111.5);
    builder.quad_to(251.0, -143.0, 197.0, -197.0);
    builder.quad_to(143.0, -251.0, 111.5, -324.0);
    builder.quad_to(80.0, -397.0, 80.0, -480.0);
    builder.quad_to(80.0, -563.0, 111.5, -636.0);
    builder.quad_to(143.0, -709.0, 197.0, -763.0);
    builder.quad_to(251.0, -817.0, 324.0, -848.5);
    builder.quad_to(397.0, -880.0, 480.0, -880.0);
    builder.quad_to(563.0, -880.0, 636.0, -848.5);
    builder.quad_to(709.0, -817.0, 763.0, -763.0);
    builder.quad_to(817.0, -709.0, 848.5, -636.0);
    builder.quad_to(880.0, -563.0, 880.0, -480.0);
    builder.quad_to(880.0, -397.0, 848.5, -324.0);
    builder.quad_to(817.0, -251.0, 763.0, -197.0);
    builder.quad_to(709.0, -143.0, 636.0, -111.5);
    builder.quad_to(563.0, -80.0, 480.0, -80.0);
    builder.quad_to(397.0, -80.0, 324.0, -111.5);
    builder.close();
    builder.move_to(480.0, -480.0);
    builder.close();
    builder.move_to(707.0, -253.0);
    builder.quad_to(800.0, -346.0, 800.0, -480.0);
    builder.quad_to(800.0, -614.0, 707.0, -707.0);
    builder.quad_to(614.0, -800.0, 480.0, -800.0);
    builder.quad_to(346.0, -800.0, 253.0, -707.0);
    builder.quad_to(160.0, -614.0, 160.0, -480.0);
    builder.quad_to(160.0, -346.0, 253.0, -253.0);
    builder.quad_to(346.0, -160.0, 480.0, -160.0);
    builder.quad_to(614.0, -160.0, 707.0, -253.0);
    builder.close();
    builder.finish()
}

/// 颜文字（工具页那一格）——Material 的 `sentiment_satisfied`。
pub(super) fn kaomoji() -> Option<Path> {
    let mut builder = PathBuilder::new();
    builder.move_to(620.0, -520.0);
    builder.quad_to(645.0, -520.0, 662.5, -537.5);
    builder.quad_to(680.0, -555.0, 680.0, -580.0);
    builder.quad_to(680.0, -605.0, 662.5, -622.5);
    builder.quad_to(645.0, -640.0, 620.0, -640.0);
    builder.quad_to(595.0, -640.0, 577.5, -622.5);
    builder.quad_to(560.0, -605.0, 560.0, -580.0);
    builder.quad_to(560.0, -555.0, 577.5, -537.5);
    builder.quad_to(595.0, -520.0, 620.0, -520.0);
    builder.close();
    builder.move_to(340.0, -520.0);
    builder.quad_to(365.0, -520.0, 382.5, -537.5);
    builder.quad_to(400.0, -555.0, 400.0, -580.0);
    builder.quad_to(400.0, -605.0, 382.5, -622.5);
    builder.quad_to(365.0, -640.0, 340.0, -640.0);
    builder.quad_to(315.0, -640.0, 297.5, -622.5);
    builder.quad_to(280.0, -605.0, 280.0, -580.0);
    builder.quad_to(280.0, -555.0, 297.5, -537.5);
    builder.quad_to(315.0, -520.0, 340.0, -520.0);
    builder.close();
    builder.move_to(603.5, -298.5);
    builder.quad_to(659.0, -337.0, 684.0, -400.0);
    builder.line_to(618.0, -400.0);
    builder.quad_to(596.0, -363.0, 559.5, -341.5);
    builder.quad_to(523.0, -320.0, 480.0, -320.0);
    builder.quad_to(437.0, -320.0, 400.5, -341.5);
    builder.quad_to(364.0, -363.0, 342.0, -400.0);
    builder.line_to(276.0, -400.0);
    builder.quad_to(301.0, -337.0, 356.5, -298.5);
    builder.quad_to(412.0, -260.0, 480.0, -260.0);
    builder.quad_to(548.0, -260.0, 603.5, -298.5);
    builder.close();
    builder.move_to(324.0, -111.5);
    builder.quad_to(251.0, -143.0, 197.0, -197.0);
    builder.quad_to(143.0, -251.0, 111.5, -324.0);
    builder.quad_to(80.0, -397.0, 80.0, -480.0);
    builder.quad_to(80.0, -563.0, 111.5, -636.0);
    builder.quad_to(143.0, -709.0, 197.0, -763.0);
    builder.quad_to(251.0, -817.0, 324.0, -848.5);
    builder.quad_to(397.0, -880.0, 480.0, -880.0);
    builder.quad_to(563.0, -880.0, 636.0, -848.5);
    builder.quad_to(709.0, -817.0, 763.0, -763.0);
    builder.quad_to(817.0, -709.0, 848.5, -636.0);
    builder.quad_to(880.0, -563.0, 880.0, -480.0);
    builder.quad_to(880.0, -397.0, 848.5, -324.0);
    builder.quad_to(817.0, -251.0, 763.0, -197.0);
    builder.quad_to(709.0, -143.0, 636.0, -111.5);
    builder.quad_to(563.0, -80.0, 480.0, -80.0);
    builder.quad_to(397.0, -80.0, 324.0, -111.5);
    builder.close();
    builder.move_to(480.0, -480.0);
    builder.close();
    builder.move_to(707.0, -253.0);
    builder.quad_to(800.0, -346.0, 800.0, -480.0);
    builder.quad_to(800.0, -614.0, 707.0, -707.0);
    builder.quad_to(614.0, -800.0, 480.0, -800.0);
    builder.quad_to(346.0, -800.0, 253.0, -707.0);
    builder.quad_to(160.0, -614.0, 160.0, -480.0);
    builder.quad_to(160.0, -346.0, 253.0, -253.0);
    builder.quad_to(346.0, -160.0, 480.0, -160.0);
    builder.quad_to(614.0, -160.0, 707.0, -253.0);
    builder.close();
    builder.finish()
}

/// 设置（工具页那一格）——Material 的 `settings`。
pub(super) fn settings() -> Option<Path> {
    let mut builder = PathBuilder::new();
    builder.move_to(370.0, -80.0);
    builder.line_to(354.0, -208.0);
    builder.quad_to(341.0, -213.0, 329.5, -220.0);
    builder.quad_to(318.0, -227.0, 307.0, -235.0);
    builder.line_to(188.0, -185.0);
    builder.line_to(78.0, -375.0);
    builder.line_to(181.0, -453.0);
    builder.quad_to(180.0, -460.0, 180.0, -466.5);
    builder.line_to(180.0, -493.5);
    builder.quad_to(180.0, -500.0, 181.0, -507.0);
    builder.line_to(78.0, -585.0);
    builder.line_to(188.0, -775.0);
    builder.line_to(307.0, -725.0);
    builder.quad_to(318.0, -733.0, 330.0, -740.0);
    builder.quad_to(342.0, -747.0, 354.0, -752.0);
    builder.line_to(370.0, -880.0);
    builder.line_to(590.0, -880.0);
    builder.line_to(606.0, -752.0);
    builder.quad_to(619.0, -747.0, 630.5, -740.0);
    builder.quad_to(642.0, -733.0, 653.0, -725.0);
    builder.line_to(772.0, -775.0);
    builder.line_to(882.0, -585.0);
    builder.line_to(779.0, -507.0);
    builder.quad_to(780.0, -500.0, 780.0, -493.5);
    builder.line_to(780.0, -466.5);
    builder.quad_to(780.0, -460.0, 778.0, -453.0);
    builder.line_to(881.0, -375.0);
    builder.line_to(771.0, -185.0);
    builder.line_to(653.0, -235.0);
    builder.quad_to(642.0, -227.0, 630.0, -220.0);
    builder.quad_to(618.0, -213.0, 606.0, -208.0);
    builder.line_to(590.0, -80.0);
    builder.line_to(370.0, -80.0);
    builder.close();
    builder.move_to(440.0, -160.0);
    builder.line_to(519.0, -160.0);
    builder.line_to(533.0, -266.0);
    builder.quad_to(564.0, -274.0, 590.5, -289.5);
    builder.quad_to(617.0, -305.0, 639.0, -327.0);
    builder.line_to(738.0, -286.0);
    builder.line_to(777.0, -354.0);
    builder.line_to(691.0, -419.0);
    builder.quad_to(696.0, -433.0, 698.0, -448.5);
    builder.quad_to(700.0, -464.0, 700.0, -480.0);
    builder.quad_to(700.0, -496.0, 698.0, -511.5);
    builder.quad_to(696.0, -527.0, 691.0, -541.0);
    builder.line_to(777.0, -606.0);
    builder.line_to(738.0, -674.0);
    builder.line_to(639.0, -632.0);
    builder.quad_to(617.0, -655.0, 590.5, -670.5);
    builder.quad_to(564.0, -686.0, 533.0, -694.0);
    builder.line_to(520.0, -800.0);
    builder.line_to(441.0, -800.0);
    builder.line_to(427.0, -694.0);
    builder.quad_to(396.0, -686.0, 369.5, -670.5);
    builder.quad_to(343.0, -655.0, 321.0, -633.0);
    builder.line_to(222.0, -674.0);
    builder.line_to(183.0, -606.0);
    builder.line_to(269.0, -542.0);
    builder.quad_to(264.0, -527.0, 262.0, -512.0);
    builder.quad_to(260.0, -497.0, 260.0, -480.0);
    builder.quad_to(260.0, -464.0, 262.0, -449.0);
    builder.quad_to(264.0, -434.0, 269.0, -419.0);
    builder.line_to(183.0, -354.0);
    builder.line_to(222.0, -286.0);
    builder.line_to(321.0, -328.0);
    builder.quad_to(343.0, -305.0, 369.5, -289.5);
    builder.quad_to(396.0, -274.0, 427.0, -266.0);
    builder.line_to(440.0, -160.0);
    builder.close();
    builder.move_to(482.0, -340.0);
    builder.quad_to(540.0, -340.0, 581.0, -381.0);
    builder.quad_to(622.0, -422.0, 622.0, -480.0);
    builder.quad_to(622.0, -538.0, 581.0, -579.0);
    builder.quad_to(540.0, -620.0, 482.0, -620.0);
    builder.quad_to(423.0, -620.0, 382.5, -579.0);
    builder.quad_to(342.0, -538.0, 342.0, -480.0);
    builder.quad_to(342.0, -422.0, 382.5, -381.0);
    builder.quad_to(423.0, -340.0, 482.0, -340.0);
    builder.close();
    builder.move_to(480.0, -480.0);
    builder.close();
    builder.finish()
}

/// 每个图标自己的包围盒（Material 那套坐标）：`(左, 上, 右, 下)`。
///
/// 画的时候按**这个框**等比缩到目标边长——960 的网格里四周是 Google 留的呼吸位，
/// 照网格缩的话画出来比要的尺寸小一圈。
pub(super) const BOXES: [(f32, f32, f32, f32); 6] = [
    (240.0, -736.0, 720.0, -240.0), // shift
    (80.0, -800.0, 880.0, -160.0),  // backspace
    (120.0, -920.0, 840.0, -120.0), // clipboard
    (80.0, -880.0, 880.0, -80.0),   // mood
    (80.0, -880.0, 880.0, -80.0),   // kaomoji
    (78.0, -880.0, 882.0, -80.0),   // settings
];
