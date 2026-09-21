//! 软键盘的布局与状态：只有数据，不含绘制。
//!
//! 怎么画（配色、几何、命中）在 [`crate::renderer::keyboard`]。键盘与候选窗同属「显示面」——
//! 由渲染器出位图、各平台只贴图，**命中测试也在渲染器一侧做**，壳只把原始触摸坐标传回来。
//! 见 `docs/design/keyboard.md`。

mod key;
mod layout;
mod panel;
mod state;

pub use key::{Key, KeyId, KeyStyle, KeyWidth};
pub use layout::{CLIPBOARD_CELLS, KeyRow, KeyboardLayout, TOOLS};
pub use panel::Panel;
pub use state::{InputMode, KeyboardState, ShiftState};
