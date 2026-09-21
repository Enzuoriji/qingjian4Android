//! 候选条整条铺开后的样子：每格在哪、多长、视口落在哪一段。
//!
//! 候选条是一条能横滚的带子，会话拿它把「滚了多远」（像素）换算成「画哪几格」
//! 与「第几屏」。**格宽只算一份**（[`Renderer::bar_cell_widths`]）——会话不自己再算一遍，
//! 否则画出来的位置与算出来的页码会各走各的。
//!
//! 这里量的是**全部**候选：一屏只画得下十来个，但要知道「一共几屏」就得知道整条多长。

use std::ops::Range;

use crate::frame::Row;
use crate::renderer::{Metrics, Renderer};
use crate::theme::Theme;

/// 整条候选铺开后的位置。
#[derive(Debug, Clone, Default)]
pub struct BarStrip {
    /// 每格的左边缘（像素，相对条子左缘，**已含左边距**）。
    lefts: Vec<f32>,

    /// 每格的宽度（像素）。
    widths: Vec<f32>,

    /// 左边距（像素）——把全局位移换算成渲染要的那个位移时要用。
    pad: f32,

    /// 整条铺开有多宽（像素，含**两侧**留白）。
    total: f32,
}

impl BarStrip {
    /// 按每格的宽度铺开。`pad` 是条子左右各留的空白（像素），`gap` 是格与格之间（像素）。
    pub(super) fn new(widths: &[f32], pad: f32, gap: f32) -> Self {
        let mut lefts = Vec::with_capacity(widths.len());
        let mut left = pad;
        for width in widths {
            lefts.push(left);
            left += width + gap;
        }
        // 走完一轮 `left` 停在「最后一格的右边 + gap」，条子总宽是最后一格的右边再加右边距
        let total = if widths.is_empty() {
            0.0
        } else {
            left - gap + pad
        };
        Self {
            lefts,
            widths: widths.to_vec(),
            pad,
            total,
        }
    }

    /// 一共几格。
    pub fn len(&self) -> usize {
        self.lefts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.lefts.is_empty()
    }

    /// 视口落在 `[scroll, scroll + viewport)` 时，该画第几格到第几格（左闭右开）。
    ///
    /// 两边各算上**只露出一半**的那格——画出来会被位图裁掉，但滚到格与格之间时
    /// 不带上它就会缺一块。滚过头（视口在带子外面）时给出的是空区间。
    pub fn slice(&self, scroll: f32, viewport: f32) -> Range<usize> {
        let first = self
            .lefts
            .iter()
            .enumerate()
            .find(|(i, left)| **left + self.widths[*i] > scroll)
            .map_or(self.lefts.len(), |(i, _)| i);
        let last = self
            .lefts
            .iter()
            .enumerate()
            .rfind(|(_, left)| **left < scroll + viewport)
            .map_or(first, |(i, _)| i + 1);
        first..last.max(first)
    }

    /// 头一个**左边没被切掉**的格——高亮认它。
    ///
    /// 滚到格子中间时，最左边那一格可能只露一条边：高亮压在它上面的话，看起来像画坏了，
    /// 而且空格上屏的会是屏幕上几乎看不见的那个词。所以高亮往后挪到第一个整格。
    pub fn first_whole(&self, scroll: f32) -> usize {
        self.lefts
            .iter()
            .position(|left| *left >= scroll)
            .unwrap_or_else(|| self.lefts.len().saturating_sub(1))
    }

    /// 把全局位移换成 [`Renderer::render_bar`] 要的那个位移。
    ///
    /// 渲染是从**传进去的第一格**开始往外铺的（铺到哪算哪，多出来的被位图裁掉），
    /// 所以喂给它的位移是「这一格相对视口左缘的位置」，不是全局那个。
    pub fn local_scroll(&self, start: usize, scroll: f32) -> f32 {
        scroll - self.lefts.get(start).copied().unwrap_or(self.pad) + self.pad
    }

    /// 视口 `viewport` 宽时最远能滚到哪儿。带子比一屏窄就是 0（没法滚）。
    pub fn max_scroll(&self, viewport: f32) -> f32 {
        (self.total - viewport).max(0.0)
    }

    /// 一共几个「停得住的位置」——页码的分母。
    ///
    /// 不是「带子有几屏长」，是**能停在几个位置上**：一屏一屏地翻，翻到最后一屏时
    /// 视口右缘正好贴住带子尾巴，那一下算最后一屏，所以分母比 `总宽 ÷ 屏宽` 小一点。
    /// 这样一路翻到底时分子能正好走到分母（不是停在 69/70）。
    pub fn screens(&self, viewport: f32) -> usize {
        if viewport <= 0.0 {
            return 1;
        }
        (self.max_scroll(viewport) / viewport).floor() as usize + 1
    }

    /// 此刻在第几屏（从 1 起）。
    pub fn screen(&self, scroll: f32, viewport: f32) -> usize {
        if viewport <= 0.0 {
            return 1;
        }
        let screen = (scroll.max(0.0) / viewport).floor() as usize + 1;
        screen.min(self.screens(viewport))
    }

    /// 第 `screen` 屏（从 1 起）对应的位移——按 `‹` `›` 翻页用。
    pub fn screen_scroll(&self, screen: usize, viewport: f32) -> f32 {
        (screen.saturating_sub(1) as f32 * viewport).min(self.max_scroll(viewport))
    }
}

impl Renderer {
    /// 整条候选铺开后的位置。
    ///
    /// 组句一变就要重算一次（量的是全部候选，见文件头）。渲染器不可用时给空的那份，
    /// 会话据此画不出候选条——与没有渲染器时的表现一致。
    pub fn bar_strip(&mut self, rows: &[Row], theme: &Theme, scale: f32) -> BarStrip {
        let metrics = Metrics { theme, scale };
        let widths = self.bar_cell_widths(rows, &metrics);
        BarStrip::new(&widths, metrics.padding(), metrics.column_gap())
    }
}
