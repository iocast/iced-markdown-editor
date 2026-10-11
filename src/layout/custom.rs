//! Code blocks drawn by a [`BlockRenderer`](crate::BlockRenderer).
//!
//! The block gets the measured area and one empty line for the caret and
//! hit-testing; the widget shows the code again when the caret enters it.

use super::{Decor, Line, Pass, metrics};
use iced::Size;

impl Pass<'_> {
    /// The rendered block `index` of `size`, at `left`, measured for `available`.
    pub(super) fn custom(
        &mut self,
        index: usize,
        indent: &[f32],
        left: f32,
        available: Size,
        size: Size,
    ) {
        let block = &self.doc.blocks[index];
        let m = metrics(block.leaf, self.style, self.z);
        let w = size.width.clamp(1.0, available.width.max(1.0));
        let h = size.height.max(m.line_height);

        self.flow.fit(h);
        let top = self.flow.y;
        self.quote_bars(block, indent, true, top, top + h);
        self.markers(index, indent, &m, top);
        self.layout.decor.push(Decor::Custom {
            block: index,
            x: left,
            y: top,
            w,
            h,
            available,
        });
        self.layout.lines.push(Line {
            block: index,
            start: 0,
            end: 0,
            xs: vec![left],
            y: top,
            height: h,
            size: m.size,
            color: m.color,
            last: true,
            page: self.flow.page,
            images: Vec::new(),
            cell: None,
        });
        self.flow.y = top + h;
    }
}
