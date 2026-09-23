//! 键盘上那些图标的路径：⇧ 大小写、⌫ 退格、工具页那几格，
//! 以及表情面板分类标签上那一排（各一个函数 + 一个包围盒）。
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

/// 「最近」那一类——Material 的 `history`。
pub(super) fn history() -> Option<Path> {
    let mut builder = PathBuilder::new();
    builder.move_to(480.0, -120.0);
    builder.quad_to(342.0, -120.0, 239.5, -211.5);
    builder.quad_to(137.0, -303.0, 122.0, -440.0);
    builder.line_to(204.0, -440.0);
    builder.quad_to(218.0, -336.0, 296.5, -268.0);
    builder.quad_to(375.0, -200.0, 480.0, -200.0);
    builder.quad_to(597.0, -200.0, 678.5, -281.5);
    builder.quad_to(760.0, -363.0, 760.0, -480.0);
    builder.quad_to(760.0, -597.0, 678.5, -678.5);
    builder.quad_to(597.0, -760.0, 480.0, -760.0);
    builder.quad_to(411.0, -760.0, 351.0, -728.0);
    builder.quad_to(291.0, -696.0, 250.0, -640.0);
    builder.line_to(360.0, -640.0);
    builder.line_to(360.0, -560.0);
    builder.line_to(120.0, -560.0);
    builder.line_to(120.0, -800.0);
    builder.line_to(200.0, -800.0);
    builder.line_to(200.0, -706.0);
    builder.quad_to(251.0, -770.0, 324.5, -805.0);
    builder.quad_to(398.0, -840.0, 480.0, -840.0);
    builder.quad_to(555.0, -840.0, 620.5, -811.5);
    builder.quad_to(686.0, -783.0, 734.5, -734.5);
    builder.quad_to(783.0, -686.0, 811.5, -620.5);
    builder.quad_to(840.0, -555.0, 840.0, -480.0);
    builder.quad_to(840.0, -405.0, 811.5, -339.5);
    builder.quad_to(783.0, -274.0, 734.5, -225.5);
    builder.quad_to(686.0, -177.0, 620.5, -148.5);
    builder.quad_to(555.0, -120.0, 480.0, -120.0);
    builder.close();
    builder.move_to(592.0, -312.0);
    builder.line_to(440.0, -464.0);
    builder.line_to(440.0, -680.0);
    builder.line_to(520.0, -680.0);
    builder.line_to(520.0, -496.0);
    builder.line_to(648.0, -368.0);
    builder.line_to(592.0, -312.0);
    builder.close();
    builder.finish()
}

/// 「People & Body」——Material 的 `emoji_people`。
pub(super) fn people() -> Option<Path> {
    let mut builder = PathBuilder::new();
    builder.move_to(360.0, -80.0);
    builder.line_to(360.0, -609.0);
    builder.quad_to(269.0, -633.0, 214.5, -709.5);
    builder.quad_to(160.0, -786.0, 160.0, -880.0);
    builder.line_to(240.0, -880.0);
    builder.quad_to(240.0, -797.0, 293.5, -738.5);
    builder.quad_to(347.0, -680.0, 430.0, -680.0);
    builder.line_to(530.0, -680.0);
    builder.quad_to(560.0, -680.0, 586.0, -669.0);
    builder.quad_to(612.0, -658.0, 633.0, -637.0);
    builder.line_to(814.0, -456.0);
    builder.line_to(758.0, -400.0);
    builder.line_to(600.0, -558.0);
    builder.line_to(600.0, -80.0);
    builder.line_to(520.0, -80.0);
    builder.line_to(520.0, -320.0);
    builder.line_to(440.0, -320.0);
    builder.line_to(440.0, -80.0);
    builder.line_to(360.0, -80.0);
    builder.close();
    builder.move_to(423.5, -743.5);
    builder.quad_to(400.0, -767.0, 400.0, -800.0);
    builder.quad_to(400.0, -833.0, 423.5, -856.5);
    builder.quad_to(447.0, -880.0, 480.0, -880.0);
    builder.quad_to(513.0, -880.0, 536.5, -856.5);
    builder.quad_to(560.0, -833.0, 560.0, -800.0);
    builder.quad_to(560.0, -767.0, 536.5, -743.5);
    builder.quad_to(513.0, -720.0, 480.0, -720.0);
    builder.quad_to(447.0, -720.0, 423.5, -743.5);
    builder.close();
    builder.finish()
}

/// 「Animals & Nature」——Material 的 `pets`。
pub(super) fn pets() -> Option<Path> {
    let mut builder = PathBuilder::new();
    builder.move_to(180.0, -475.0);
    builder.quad_to(138.0, -475.0, 109.0, -504.0);
    builder.quad_to(80.0, -533.0, 80.0, -575.0);
    builder.quad_to(80.0, -617.0, 109.0, -646.0);
    builder.quad_to(138.0, -675.0, 180.0, -675.0);
    builder.quad_to(222.0, -675.0, 251.0, -646.0);
    builder.quad_to(280.0, -617.0, 280.0, -575.0);
    builder.quad_to(280.0, -533.0, 251.0, -504.0);
    builder.quad_to(222.0, -475.0, 180.0, -475.0);
    builder.close();
    builder.move_to(289.0, -664.0);
    builder.quad_to(260.0, -693.0, 260.0, -735.0);
    builder.quad_to(260.0, -777.0, 289.0, -806.0);
    builder.quad_to(318.0, -835.0, 360.0, -835.0);
    builder.quad_to(402.0, -835.0, 431.0, -806.0);
    builder.quad_to(460.0, -777.0, 460.0, -735.0);
    builder.quad_to(460.0, -693.0, 431.0, -664.0);
    builder.quad_to(402.0, -635.0, 360.0, -635.0);
    builder.quad_to(318.0, -635.0, 289.0, -664.0);
    builder.close();
    builder.move_to(529.0, -664.0);
    builder.quad_to(500.0, -693.0, 500.0, -735.0);
    builder.quad_to(500.0, -777.0, 529.0, -806.0);
    builder.quad_to(558.0, -835.0, 600.0, -835.0);
    builder.quad_to(642.0, -835.0, 671.0, -806.0);
    builder.quad_to(700.0, -777.0, 700.0, -735.0);
    builder.quad_to(700.0, -693.0, 671.0, -664.0);
    builder.quad_to(642.0, -635.0, 600.0, -635.0);
    builder.quad_to(558.0, -635.0, 529.0, -664.0);
    builder.close();
    builder.move_to(780.0, -475.0);
    builder.quad_to(738.0, -475.0, 709.0, -504.0);
    builder.quad_to(680.0, -533.0, 680.0, -575.0);
    builder.quad_to(680.0, -617.0, 709.0, -646.0);
    builder.quad_to(738.0, -675.0, 780.0, -675.0);
    builder.quad_to(822.0, -675.0, 851.0, -646.0);
    builder.quad_to(880.0, -617.0, 880.0, -575.0);
    builder.quad_to(880.0, -533.0, 851.0, -504.0);
    builder.quad_to(822.0, -475.0, 780.0, -475.0);
    builder.close();
    builder.move_to(266.0, -75.0);
    builder.quad_to(221.0, -75.0, 190.5, -109.5);
    builder.quad_to(160.0, -144.0, 160.0, -191.0);
    builder.quad_to(160.0, -243.0, 195.5, -282.0);
    builder.quad_to(231.0, -321.0, 266.0, -359.0);
    builder.quad_to(295.0, -390.0, 316.0, -426.5);
    builder.quad_to(337.0, -463.0, 366.0, -495.0);
    builder.quad_to(388.0, -521.0, 417.0, -538.0);
    builder.quad_to(446.0, -555.0, 480.0, -555.0);
    builder.quad_to(514.0, -555.0, 543.0, -539.0);
    builder.quad_to(572.0, -523.0, 594.0, -497.0);
    builder.quad_to(622.0, -465.0, 643.5, -428.0);
    builder.quad_to(665.0, -391.0, 694.0, -359.0);
    builder.quad_to(729.0, -321.0, 764.5, -282.0);
    builder.quad_to(800.0, -243.0, 800.0, -191.0);
    builder.quad_to(800.0, -144.0, 769.5, -109.5);
    builder.quad_to(739.0, -75.0, 694.0, -75.0);
    builder.quad_to(640.0, -75.0, 587.0, -84.0);
    builder.quad_to(534.0, -93.0, 480.0, -93.0);
    builder.quad_to(426.0, -93.0, 373.0, -84.0);
    builder.quad_to(320.0, -75.0, 266.0, -75.0);
    builder.close();
    builder.finish()
}

/// 「Food & Drink」——Material 的 `cake`。
pub(super) fn cake() -> Option<Path> {
    let mut builder = PathBuilder::new();
    builder.move_to(160.0, -80.0);
    builder.quad_to(143.0, -80.0, 131.5, -91.5);
    builder.quad_to(120.0, -103.0, 120.0, -120.0);
    builder.line_to(120.0, -320.0);
    builder.quad_to(120.0, -353.0, 143.5, -376.5);
    builder.quad_to(167.0, -400.0, 200.0, -400.0);
    builder.line_to(200.0, -560.0);
    builder.quad_to(200.0, -593.0, 223.5, -616.5);
    builder.quad_to(247.0, -640.0, 280.0, -640.0);
    builder.line_to(440.0, -640.0);
    builder.line_to(440.0, -698.0);
    builder.quad_to(422.0, -710.0, 411.0, -727.0);
    builder.quad_to(400.0, -744.0, 400.0, -768.0);
    builder.quad_to(400.0, -783.0, 406.0, -797.5);
    builder.quad_to(412.0, -812.0, 424.0, -824.0);
    builder.line_to(480.0, -880.0);
    builder.line_to(536.0, -824.0);
    builder.quad_to(548.0, -812.0, 554.0, -797.5);
    builder.quad_to(560.0, -783.0, 560.0, -768.0);
    builder.quad_to(560.0, -744.0, 549.0, -727.0);
    builder.quad_to(538.0, -710.0, 520.0, -698.0);
    builder.line_to(520.0, -640.0);
    builder.line_to(680.0, -640.0);
    builder.quad_to(713.0, -640.0, 736.5, -616.5);
    builder.quad_to(760.0, -593.0, 760.0, -560.0);
    builder.line_to(760.0, -400.0);
    builder.quad_to(793.0, -400.0, 816.5, -376.5);
    builder.quad_to(840.0, -353.0, 840.0, -320.0);
    builder.line_to(840.0, -120.0);
    builder.quad_to(840.0, -103.0, 828.5, -91.5);
    builder.quad_to(817.0, -80.0, 800.0, -80.0);
    builder.line_to(160.0, -80.0);
    builder.close();
    builder.move_to(280.0, -400.0);
    builder.line_to(680.0, -400.0);
    builder.line_to(680.0, -560.0);
    builder.line_to(280.0, -560.0);
    builder.line_to(280.0, -400.0);
    builder.close();
    builder.move_to(200.0, -160.0);
    builder.line_to(760.0, -160.0);
    builder.line_to(760.0, -320.0);
    builder.line_to(200.0, -320.0);
    builder.line_to(200.0, -160.0);
    builder.close();
    builder.move_to(280.0, -400.0);
    builder.line_to(680.0, -400.0);
    builder.line_to(280.0, -400.0);
    builder.close();
    builder.move_to(200.0, -160.0);
    builder.line_to(760.0, -160.0);
    builder.line_to(200.0, -160.0);
    builder.close();
    builder.move_to(760.0, -400.0);
    builder.line_to(200.0, -400.0);
    builder.line_to(760.0, -400.0);
    builder.close();
    builder.finish()
}

/// 「Travel & Places」——Material 的 `directions_car`。
pub(super) fn car() -> Option<Path> {
    let mut builder = PathBuilder::new();
    builder.move_to(240.0, -200.0);
    builder.line_to(240.0, -160.0);
    builder.quad_to(240.0, -143.0, 228.5, -131.5);
    builder.quad_to(217.0, -120.0, 200.0, -120.0);
    builder.line_to(160.0, -120.0);
    builder.quad_to(143.0, -120.0, 131.5, -131.5);
    builder.quad_to(120.0, -143.0, 120.0, -160.0);
    builder.line_to(120.0, -480.0);
    builder.line_to(204.0, -720.0);
    builder.quad_to(210.0, -738.0, 225.5, -749.0);
    builder.quad_to(241.0, -760.0, 260.0, -760.0);
    builder.line_to(700.0, -760.0);
    builder.quad_to(719.0, -760.0, 734.5, -749.0);
    builder.quad_to(750.0, -738.0, 756.0, -720.0);
    builder.line_to(840.0, -480.0);
    builder.line_to(840.0, -160.0);
    builder.quad_to(840.0, -143.0, 828.5, -131.5);
    builder.quad_to(817.0, -120.0, 800.0, -120.0);
    builder.line_to(760.0, -120.0);
    builder.quad_to(743.0, -120.0, 731.5, -131.5);
    builder.quad_to(720.0, -143.0, 720.0, -160.0);
    builder.line_to(720.0, -200.0);
    builder.line_to(240.0, -200.0);
    builder.close();
    builder.move_to(232.0, -560.0);
    builder.line_to(728.0, -560.0);
    builder.line_to(686.0, -680.0);
    builder.line_to(274.0, -680.0);
    builder.line_to(232.0, -560.0);
    builder.close();
    builder.move_to(200.0, -480.0);
    builder.line_to(200.0, -280.0);
    builder.line_to(200.0, -480.0);
    builder.close();
    builder.move_to(300.0, -320.0);
    builder.quad_to(325.0, -320.0, 342.5, -337.5);
    builder.quad_to(360.0, -355.0, 360.0, -380.0);
    builder.quad_to(360.0, -405.0, 342.5, -422.5);
    builder.quad_to(325.0, -440.0, 300.0, -440.0);
    builder.quad_to(275.0, -440.0, 257.5, -422.5);
    builder.quad_to(240.0, -405.0, 240.0, -380.0);
    builder.quad_to(240.0, -355.0, 257.5, -337.5);
    builder.quad_to(275.0, -320.0, 300.0, -320.0);
    builder.close();
    builder.move_to(660.0, -320.0);
    builder.quad_to(685.0, -320.0, 702.5, -337.5);
    builder.quad_to(720.0, -355.0, 720.0, -380.0);
    builder.quad_to(720.0, -405.0, 702.5, -422.5);
    builder.quad_to(685.0, -440.0, 660.0, -440.0);
    builder.quad_to(635.0, -440.0, 617.5, -422.5);
    builder.quad_to(600.0, -405.0, 600.0, -380.0);
    builder.quad_to(600.0, -355.0, 617.5, -337.5);
    builder.quad_to(635.0, -320.0, 660.0, -320.0);
    builder.close();
    builder.move_to(200.0, -280.0);
    builder.line_to(760.0, -280.0);
    builder.line_to(760.0, -480.0);
    builder.line_to(200.0, -480.0);
    builder.line_to(200.0, -280.0);
    builder.close();
    builder.finish()
}

/// 「Activities」——Material 的 `sports_basketball`。
pub(super) fn ball() -> Option<Path> {
    let mut builder = PathBuilder::new();
    builder.move_to(162.0, -520.0);
    builder.line_to(276.0, -520.0);
    builder.quad_to(270.0, -558.0, 253.0, -591.0);
    builder.quad_to(236.0, -624.0, 210.0, -650.0);
    builder.quad_to(192.0, -621.0, 179.5, -588.5);
    builder.quad_to(167.0, -556.0, 162.0, -520.0);
    builder.close();
    builder.move_to(684.0, -520.0);
    builder.line_to(798.0, -520.0);
    builder.quad_to(793.0, -556.0, 780.5, -588.5);
    builder.quad_to(768.0, -621.0, 750.0, -650.0);
    builder.quad_to(724.0, -624.0, 707.0, -591.0);
    builder.quad_to(690.0, -558.0, 684.0, -520.0);
    builder.close();
    builder.move_to(210.0, -310.0);
    builder.quad_to(236.0, -336.0, 253.0, -369.0);
    builder.quad_to(270.0, -402.0, 276.0, -440.0);
    builder.line_to(162.0, -440.0);
    builder.quad_to(167.0, -404.0, 179.5, -371.5);
    builder.quad_to(192.0, -339.0, 210.0, -310.0);
    builder.close();
    builder.move_to(750.0, -310.0);
    builder.quad_to(768.0, -339.0, 780.5, -371.5);
    builder.quad_to(793.0, -404.0, 798.0, -440.0);
    builder.line_to(684.0, -440.0);
    builder.quad_to(690.0, -402.0, 707.0, -369.0);
    builder.quad_to(724.0, -336.0, 750.0, -310.0);
    builder.close();
    builder.move_to(358.0, -520.0);
    builder.line_to(440.0, -520.0);
    builder.line_to(440.0, -798.0);
    builder.quad_to(387.0, -790.0, 341.5, -768.5);
    builder.quad_to(296.0, -747.0, 260.0, -712.0);
    builder.quad_to(299.0, -674.0, 324.5, -625.5);
    builder.quad_to(350.0, -577.0, 358.0, -520.0);
    builder.close();
    builder.move_to(520.0, -520.0);
    builder.line_to(602.0, -520.0);
    builder.quad_to(610.0, -577.0, 635.5, -625.5);
    builder.quad_to(661.0, -674.0, 700.0, -712.0);
    builder.quad_to(664.0, -747.0, 618.5, -768.5);
    builder.quad_to(573.0, -790.0, 520.0, -798.0);
    builder.line_to(520.0, -520.0);
    builder.close();
    builder.move_to(440.0, -162.0);
    builder.line_to(440.0, -440.0);
    builder.line_to(358.0, -440.0);
    builder.quad_to(350.0, -383.0, 324.5, -334.5);
    builder.quad_to(299.0, -286.0, 260.0, -248.0);
    builder.quad_to(296.0, -213.0, 341.5, -191.5);
    builder.quad_to(387.0, -170.0, 440.0, -162.0);
    builder.close();
    builder.move_to(520.0, -162.0);
    builder.quad_to(573.0, -170.0, 618.5, -191.5);
    builder.quad_to(664.0, -213.0, 700.0, -248.0);
    builder.quad_to(661.0, -286.0, 635.5, -334.5);
    builder.quad_to(610.0, -383.0, 602.0, -440.0);
    builder.line_to(520.0, -440.0);
    builder.line_to(520.0, -162.0);
    builder.close();
    builder.move_to(480.0, -480.0);
    builder.close();
    builder.move_to(480.0, -80.0);
    builder.quad_to(397.0, -80.0, 324.0, -111.5);
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
    builder.close();
    builder.finish()
}

/// 「Objects」——Material 的 `emoji_objects`。
pub(super) fn objects() -> Option<Path> {
    let mut builder = PathBuilder::new();
    builder.move_to(480.0, -80.0);
    builder.quad_to(454.0, -80.0, 433.0, -92.5);
    builder.quad_to(412.0, -105.0, 400.0, -126.0);
    builder.quad_to(367.0, -126.0, 343.5, -149.5);
    builder.quad_to(320.0, -173.0, 320.0, -206.0);
    builder.line_to(320.0, -348.0);
    builder.quad_to(261.0, -387.0, 225.5, -451.0);
    builder.quad_to(190.0, -515.0, 190.0, -590.0);
    builder.quad_to(190.0, -711.0, 274.5, -795.5);
    builder.quad_to(359.0, -880.0, 480.0, -880.0);
    builder.quad_to(601.0, -880.0, 685.5, -795.5);
    builder.quad_to(770.0, -711.0, 770.0, -590.0);
    builder.quad_to(770.0, -513.0, 734.5, -450.0);
    builder.quad_to(699.0, -387.0, 640.0, -348.0);
    builder.line_to(640.0, -206.0);
    builder.quad_to(640.0, -173.0, 616.5, -149.5);
    builder.quad_to(593.0, -126.0, 560.0, -126.0);
    builder.quad_to(548.0, -105.0, 527.0, -92.5);
    builder.quad_to(506.0, -80.0, 480.0, -80.0);
    builder.close();
    builder.move_to(400.0, -206.0);
    builder.line_to(560.0, -206.0);
    builder.line_to(560.0, -242.0);
    builder.line_to(400.0, -242.0);
    builder.line_to(400.0, -206.0);
    builder.close();
    builder.move_to(400.0, -282.0);
    builder.line_to(560.0, -282.0);
    builder.line_to(560.0, -320.0);
    builder.line_to(400.0, -320.0);
    builder.line_to(400.0, -282.0);
    builder.close();
    builder.move_to(392.0, -400.0);
    builder.line_to(450.0, -400.0);
    builder.line_to(450.0, -508.0);
    builder.line_to(362.0, -596.0);
    builder.line_to(404.0, -638.0);
    builder.line_to(480.0, -562.0);
    builder.line_to(556.0, -638.0);
    builder.line_to(598.0, -596.0);
    builder.line_to(510.0, -508.0);
    builder.line_to(510.0, -400.0);
    builder.line_to(568.0, -400.0);
    builder.quad_to(622.0, -426.0, 656.0, -476.5);
    builder.quad_to(690.0, -527.0, 690.0, -590.0);
    builder.quad_to(690.0, -678.0, 629.0, -739.0);
    builder.quad_to(568.0, -800.0, 480.0, -800.0);
    builder.quad_to(392.0, -800.0, 331.0, -739.0);
    builder.quad_to(270.0, -678.0, 270.0, -590.0);
    builder.quad_to(270.0, -527.0, 304.0, -476.5);
    builder.quad_to(338.0, -426.0, 392.0, -400.0);
    builder.close();
    builder.move_to(480.0, -562.0);
    builder.close();
    builder.move_to(480.0, -600.0);
    builder.close();
    builder.finish()
}

/// 「Symbols」——Material 的 `emoji_symbols`。
pub(super) fn symbols() -> Option<Path> {
    let mut builder = PathBuilder::new();
    builder.move_to(120.0, -800.0);
    builder.line_to(120.0, -880.0);
    builder.line_to(440.0, -880.0);
    builder.line_to(440.0, -800.0);
    builder.line_to(120.0, -800.0);
    builder.close();
    builder.move_to(240.0, -520.0);
    builder.line_to(240.0, -680.0);
    builder.line_to(120.0, -680.0);
    builder.line_to(120.0, -760.0);
    builder.line_to(440.0, -760.0);
    builder.line_to(440.0, -680.0);
    builder.line_to(320.0, -680.0);
    builder.line_to(320.0, -520.0);
    builder.line_to(240.0, -520.0);
    builder.close();
    builder.move_to(548.0, -96.0);
    builder.line_to(492.0, -152.0);
    builder.line_to(804.0, -464.0);
    builder.line_to(860.0, -408.0);
    builder.line_to(548.0, -96.0);
    builder.close();
    builder.move_to(537.0, -337.0);
    builder.quad_to(520.0, -354.0, 520.0, -380.0);
    builder.quad_to(520.0, -406.0, 537.0, -423.0);
    builder.quad_to(554.0, -440.0, 580.0, -440.0);
    builder.quad_to(606.0, -440.0, 623.0, -423.0);
    builder.quad_to(640.0, -406.0, 640.0, -380.0);
    builder.quad_to(640.0, -354.0, 623.0, -337.0);
    builder.quad_to(606.0, -320.0, 580.0, -320.0);
    builder.quad_to(554.0, -320.0, 537.0, -337.0);
    builder.close();
    builder.move_to(737.0, -137.0);
    builder.quad_to(720.0, -154.0, 720.0, -180.0);
    builder.quad_to(720.0, -206.0, 737.0, -223.0);
    builder.quad_to(754.0, -240.0, 780.0, -240.0);
    builder.quad_to(806.0, -240.0, 823.0, -223.0);
    builder.quad_to(840.0, -206.0, 840.0, -180.0);
    builder.quad_to(840.0, -154.0, 823.0, -137.0);
    builder.quad_to(806.0, -120.0, 780.0, -120.0);
    builder.quad_to(754.0, -120.0, 737.0, -137.0);
    builder.close();
    builder.move_to(549.5, -549.5);
    builder.quad_to(520.0, -579.0, 520.0, -620.0);
    builder.quad_to(520.0, -661.0, 549.5, -691.5);
    builder.quad_to(579.0, -722.0, 620.0, -722.0);
    builder.quad_to(632.0, -722.0, 641.5, -720.5);
    builder.quad_to(651.0, -719.0, 660.0, -716.0);
    builder.line_to(660.0, -840.0);
    builder.quad_to(660.0, -857.0, 671.5, -868.5);
    builder.quad_to(683.0, -880.0, 700.0, -880.0);
    builder.line_to(840.0, -880.0);
    builder.line_to(840.0, -800.0);
    builder.line_to(720.0, -800.0);
    builder.line_to(720.0, -620.0);
    builder.quad_to(720.0, -579.0, 690.5, -549.5);
    builder.quad_to(661.0, -520.0, 620.0, -520.0);
    builder.quad_to(579.0, -520.0, 549.5, -549.5);
    builder.close();
    builder.move_to(220.0, -80.0);
    builder.quad_to(179.0, -80.0, 149.5, -110.5);
    builder.quad_to(120.0, -141.0, 120.0, -182.0);
    builder.quad_to(120.0, -200.0, 127.5, -218.5);
    builder.quad_to(135.0, -237.0, 150.0, -252.0);
    builder.line_to(192.0, -294.0);
    builder.line_to(178.0, -308.0);
    builder.quad_to(163.0, -323.0, 155.5, -340.5);
    builder.quad_to(148.0, -358.0, 148.0, -378.0);
    builder.quad_to(148.0, -419.0, 177.5, -448.5);
    builder.quad_to(207.0, -478.0, 248.0, -478.0);
    builder.quad_to(289.0, -478.0, 318.5, -448.5);
    builder.quad_to(348.0, -419.0, 348.0, -378.0);
    builder.quad_to(348.0, -358.0, 341.5, -340.5);
    builder.quad_to(335.0, -323.0, 320.0, -308.0);
    builder.line_to(306.0, -294.0);
    builder.line_to(334.0, -266.0);
    builder.line_to(390.0, -322.0);
    builder.line_to(446.0, -264.0);
    builder.line_to(390.0, -208.0);
    builder.line_to(446.0, -152.0);
    builder.line_to(390.0, -96.0);
    builder.line_to(334.0, -152.0);
    builder.line_to(292.0, -110.0);
    builder.quad_to(277.0, -95.0, 258.5, -87.5);
    builder.quad_to(240.0, -80.0, 220.0, -80.0);
    builder.close();
    builder.move_to(248.0, -350.0);
    builder.line_to(262.0, -364.0);
    builder.quad_to(265.0, -367.0, 266.5, -370.0);
    builder.quad_to(268.0, -373.0, 268.0, -378.0);
    builder.quad_to(268.0, -387.0, 262.0, -392.5);
    builder.quad_to(256.0, -398.0, 248.0, -398.0);
    builder.quad_to(240.0, -398.0, 234.0, -392.5);
    builder.quad_to(228.0, -387.0, 228.0, -378.0);
    builder.quad_to(228.0, -375.0, 229.5, -371.0);
    builder.quad_to(231.0, -367.0, 234.0, -364.0);
    builder.line_to(248.0, -350.0);
    builder.close();
    builder.move_to(218.0, -160.0);
    builder.quad_to(221.0, -160.0, 226.0, -161.5);
    builder.quad_to(231.0, -163.0, 234.0, -166.0);
    builder.line_to(278.0, -208.0);
    builder.line_to(250.0, -236.0);
    builder.line_to(206.0, -194.0);
    builder.quad_to(203.0, -191.0, 201.5, -187.0);
    builder.quad_to(200.0, -183.0, 200.0, -178.0);
    builder.quad_to(200.0, -170.0, 205.0, -165.0);
    builder.quad_to(210.0, -160.0, 218.0, -160.0);
    builder.close();
    builder.finish()
}

/// 「Flags」——Material 的 `flag`。
pub(super) fn flag() -> Option<Path> {
    let mut builder = PathBuilder::new();
    builder.move_to(200.0, -120.0);
    builder.line_to(200.0, -800.0);
    builder.line_to(560.0, -800.0);
    builder.line_to(576.0, -720.0);
    builder.line_to(800.0, -720.0);
    builder.line_to(800.0, -320.0);
    builder.line_to(520.0, -320.0);
    builder.line_to(504.0, -400.0);
    builder.line_to(280.0, -400.0);
    builder.line_to(280.0, -120.0);
    builder.line_to(200.0, -120.0);
    builder.close();
    builder.move_to(500.0, -560.0);
    builder.close();
    builder.move_to(586.0, -400.0);
    builder.line_to(720.0, -400.0);
    builder.line_to(720.0, -640.0);
    builder.line_to(510.0, -640.0);
    builder.line_to(494.0, -720.0);
    builder.line_to(280.0, -720.0);
    builder.line_to(280.0, -480.0);
    builder.line_to(570.0, -480.0);
    builder.line_to(586.0, -400.0);
    builder.close();
    builder.finish()
}

/// 剪贴板锁上——Material 的 `lock`。
pub(super) fn lock() -> Option<Path> {
    let mut builder = PathBuilder::new();
    builder.move_to(240.0, -80.0);
    builder.quad_to(207.0, -80.0, 183.5, -103.5);
    builder.quad_to(160.0, -127.0, 160.0, -160.0);
    builder.line_to(160.0, -560.0);
    builder.quad_to(160.0, -593.0, 183.5, -616.5);
    builder.quad_to(207.0, -640.0, 240.0, -640.0);
    builder.line_to(280.0, -640.0);
    builder.line_to(280.0, -720.0);
    builder.quad_to(280.0, -803.0, 338.5, -861.5);
    builder.quad_to(397.0, -920.0, 480.0, -920.0);
    builder.quad_to(563.0, -920.0, 621.5, -861.5);
    builder.quad_to(680.0, -803.0, 680.0, -720.0);
    builder.line_to(680.0, -640.0);
    builder.line_to(720.0, -640.0);
    builder.quad_to(753.0, -640.0, 776.5, -616.5);
    builder.quad_to(800.0, -593.0, 800.0, -560.0);
    builder.line_to(800.0, -160.0);
    builder.quad_to(800.0, -127.0, 776.5, -103.5);
    builder.quad_to(753.0, -80.0, 720.0, -80.0);
    builder.line_to(240.0, -80.0);
    builder.close();
    builder.move_to(240.0, -160.0);
    builder.line_to(720.0, -160.0);
    builder.line_to(720.0, -560.0);
    builder.line_to(240.0, -560.0);
    builder.line_to(240.0, -160.0);
    builder.close();
    builder.move_to(536.5, -303.5);
    builder.quad_to(560.0, -327.0, 560.0, -360.0);
    builder.quad_to(560.0, -393.0, 536.5, -416.5);
    builder.quad_to(513.0, -440.0, 480.0, -440.0);
    builder.quad_to(447.0, -440.0, 423.5, -416.5);
    builder.quad_to(400.0, -393.0, 400.0, -360.0);
    builder.quad_to(400.0, -327.0, 423.5, -303.5);
    builder.quad_to(447.0, -280.0, 480.0, -280.0);
    builder.quad_to(513.0, -280.0, 536.5, -303.5);
    builder.close();
    builder.move_to(360.0, -640.0);
    builder.line_to(600.0, -640.0);
    builder.line_to(600.0, -720.0);
    builder.quad_to(600.0, -770.0, 565.0, -805.0);
    builder.quad_to(530.0, -840.0, 480.0, -840.0);
    builder.quad_to(430.0, -840.0, 395.0, -805.0);
    builder.quad_to(360.0, -770.0, 360.0, -720.0);
    builder.line_to(360.0, -640.0);
    builder.close();
    builder.move_to(240.0, -160.0);
    builder.line_to(240.0, -560.0);
    builder.line_to(240.0, -160.0);
    builder.close();
    builder.finish()
}

/// 剪贴板没锁——Material 的 `lock_open`。
pub(super) fn unlock() -> Option<Path> {
    let mut builder = PathBuilder::new();
    builder.move_to(240.0, -640.0);
    builder.line_to(600.0, -640.0);
    builder.line_to(600.0, -720.0);
    builder.quad_to(600.0, -770.0, 565.0, -805.0);
    builder.quad_to(530.0, -840.0, 480.0, -840.0);
    builder.quad_to(430.0, -840.0, 395.0, -805.0);
    builder.quad_to(360.0, -770.0, 360.0, -720.0);
    builder.line_to(280.0, -720.0);
    builder.quad_to(280.0, -803.0, 338.5, -861.5);
    builder.quad_to(397.0, -920.0, 480.0, -920.0);
    builder.quad_to(563.0, -920.0, 621.5, -861.5);
    builder.quad_to(680.0, -803.0, 680.0, -720.0);
    builder.line_to(680.0, -640.0);
    builder.line_to(720.0, -640.0);
    builder.quad_to(753.0, -640.0, 776.5, -616.5);
    builder.quad_to(800.0, -593.0, 800.0, -560.0);
    builder.line_to(800.0, -160.0);
    builder.quad_to(800.0, -127.0, 776.5, -103.5);
    builder.quad_to(753.0, -80.0, 720.0, -80.0);
    builder.line_to(240.0, -80.0);
    builder.quad_to(207.0, -80.0, 183.5, -103.5);
    builder.quad_to(160.0, -127.0, 160.0, -160.0);
    builder.line_to(160.0, -560.0);
    builder.quad_to(160.0, -593.0, 183.5, -616.5);
    builder.quad_to(207.0, -640.0, 240.0, -640.0);
    builder.close();
    builder.move_to(240.0, -160.0);
    builder.line_to(720.0, -160.0);
    builder.line_to(720.0, -560.0);
    builder.line_to(240.0, -560.0);
    builder.line_to(240.0, -160.0);
    builder.close();
    builder.move_to(536.5, -303.5);
    builder.quad_to(560.0, -327.0, 560.0, -360.0);
    builder.quad_to(560.0, -393.0, 536.5, -416.5);
    builder.quad_to(513.0, -440.0, 480.0, -440.0);
    builder.quad_to(447.0, -440.0, 423.5, -416.5);
    builder.quad_to(400.0, -393.0, 400.0, -360.0);
    builder.quad_to(400.0, -327.0, 423.5, -303.5);
    builder.quad_to(447.0, -280.0, 480.0, -280.0);
    builder.quad_to(513.0, -280.0, 536.5, -303.5);
    builder.close();
    builder.move_to(240.0, -160.0);
    builder.line_to(240.0, -560.0);
    builder.line_to(240.0, -160.0);
    builder.close();
    builder.finish()
}

/// 每个图标自己的包围盒（Material 那套坐标）：`(左, 上, 右, 下)`。
///
/// 画的时候按**这个框**等比缩到目标边长——960 的网格里四周是 Google 留的呼吸位，
/// 照网格缩的话画出来比要的尺寸小一圈。
pub(super) const BOXES: [(f32, f32, f32, f32); 17] = [
    (240.0, -736.0, 720.0, -240.0), // shift
    (80.0, -800.0, 880.0, -160.0),  // backspace
    (120.0, -920.0, 840.0, -120.0), // clipboard
    (80.0, -880.0, 880.0, -80.0),   // mood
    (80.0, -880.0, 880.0, -80.0),   // kaomoji
    (78.0, -880.0, 882.0, -80.0),   // settings
    (120.0, -840.0, 840.0, -120.0), // history
    (160.0, -880.0, 814.0, -80.0),  // people
    (80.0, -835.0, 880.0, -75.0),   // pets
    (120.0, -880.0, 840.0, -80.0),  // cake
    (120.0, -760.0, 840.0, -120.0), // car
    (80.0, -880.0, 880.0, -80.0),   // ball
    (190.0, -880.0, 770.0, -80.0),  // objects
    (120.0, -880.0, 860.0, -80.0),  // symbols
    (200.0, -800.0, 800.0, -120.0), // flag
    (160.0, -920.0, 800.0, -80.0),  // lock
    (160.0, -920.0, 800.0, -80.0),  // unlock
];
