//! Caret geometry, hit-testing and vertical caret movement.

use super::{Layout, TaskBox};
use crate::doc::Pos;

impl Layout {
    /// Index of the line that displays `pos`.
    pub fn line_for(&self, pos: Pos) -> Option<usize> {
        let mut fallback = None;
        for (index, line) in self.lines.iter().enumerate() {
            if line.block == pos.block {
                if pos.offset >= line.start && (pos.offset < line.end || line.last) {
                    return Some(index);
                }
                fallback = Some(index);
            } else if line.block > pos.block {
                break;
            }
        }
        fallback
    }

    /// Caret geometry `(x, y, height)`.
    pub fn caret(&self, pos: Pos) -> Option<(f32, f32, f32)> {
        let line = &self.lines[self.line_for(pos)?];
        let k = pos.offset.saturating_sub(line.start).min(line.xs.len() - 1);
        Some((line.xs[k], line.y, line.height))
    }

    /// Page (0-based) that shows `pos`.
    pub fn page_of(&self, pos: Pos) -> usize {
        self.line_for(pos)
            .map(|index| self.lines[index].page)
            .unwrap_or(0)
    }

    /// The task checkbox at a point, if any.
    pub fn task_at(&self, x: f32, y: f32) -> Option<TaskBox> {
        self.tasks.iter().copied().find(|task| task.contains(x, y))
    }

    /// Last caret offset on a line, relative to its start. Wrapped lines end
    /// before their break character.
    fn line_max(&self, index: usize) -> usize {
        let line = &self.lines[index];
        let len = line.end - line.start;
        if line.last || len == 0 { len } else { len - 1 }
    }

    /// Position closest to `x` on the given line.
    pub fn hit_line(&self, index: usize, x: f32) -> Pos {
        let line = &self.lines[index];
        let max = self.line_max(index);
        let mut best = 0;
        let mut best_distance = f32::INFINITY;
        for k in 0..=max {
            let distance = (line.xs[k] - x).abs();
            if distance < best_distance {
                best_distance = distance;
                best = k;
            }
        }
        Pos::new(line.block, line.start + best)
    }

    /// Vertical extent of a line, or of the table cell showing it.
    fn band(&self, index: usize) -> (f32, f32) {
        let line = &self.lines[index];
        match line.cell {
            Some(cell) => (cell.top, cell.bottom),
            None => (line.y, line.y + line.height),
        }
    }

    /// First and last line of the cell closest to `x`, in the table row
    /// of line `index`.
    fn cell_lines(&self, index: usize, x: f32) -> (usize, usize) {
        let Some(row) = self.lines[index].cell.map(|cell| cell.row) else {
            return (index, index);
        };
        let in_row = |i: usize| self.lines[i].cell.is_some_and(|cell| cell.row == row);
        let mut first = index;
        while first > 0 && in_row(first - 1) {
            first -= 1;
        }
        let mut end = index + 1;
        while end < self.lines.len() && in_row(end) {
            end += 1;
        }
        let distance = |i: usize| self.lines[i].cell.map_or(f32::INFINITY, |c| c.distance(x));
        let best = (first..end)
            .min_by(|a, b| distance(*a).total_cmp(&distance(*b)))
            .unwrap_or(index);
        let block = self.lines[best].block;
        let start = (first..end)
            .find(|i| self.lines[*i].block == block)
            .unwrap_or(best);
        let last = (first..end)
            .rev()
            .find(|i| self.lines[*i].block == block)
            .unwrap_or(best);
        (start, last)
    }

    /// Position closest to a point.
    pub fn hit(&self, x: f32, y: f32) -> Pos {
        if self.lines.is_empty() {
            return Pos::default();
        }
        let mut index = self.lines.len() - 1;
        for i in 0..self.lines.len() {
            if y < self.band(i).1 {
                index = i;
                break;
            }
        }
        if self.lines[index].cell.is_some() {
            let (first, last) = self.cell_lines(index, x);
            index = (first..=last)
                .find(|i| y < self.lines[*i].y + self.lines[*i].height)
                .unwrap_or(last);
        }
        self.hit_line(index, x)
    }

    /// The line above or below line `index`; in tables the line of the
    /// cell above or below, closest to `x`.
    fn step(&self, index: usize, x: f32, down: bool) -> Option<usize> {
        let line = &self.lines[index];
        let count = self.lines.len();
        if down {
            if line.cell.is_some() && index + 1 < count && self.lines[index + 1].block == line.block
            {
                return Some(index + 1);
            }
            let bottom = self.band(index).1;
            let next = (index + 1..count).find(|i| self.band(*i).0 >= bottom - 0.5)?;
            Some(self.cell_lines(next, x).0)
        } else {
            if line.cell.is_some() && index > 0 && self.lines[index - 1].block == line.block {
                return Some(index - 1);
            }
            let top = self.band(index).0;
            let previous = (0..index).rev().find(|i| self.band(*i).1 <= top + 0.5)?;
            Some(self.cell_lines(previous, x).1)
        }
    }

    /// Moves `delta` lines up (negative) or down, keeping `goal_x`.
    pub fn vertical(&self, pos: Pos, goal_x: f32, delta: i32) -> Option<Pos> {
        let mut index = self.line_for(pos)?;
        for _ in 0..delta.unsigned_abs() {
            match self.step(index, goal_x, delta > 0) {
                Some(next) => index = next,
                None => break,
            }
        }
        Some(self.hit_line(index, goal_x))
    }

    pub fn line_start(&self, pos: Pos) -> Pos {
        match self.line_for(pos) {
            Some(index) => Pos::new(pos.block, self.lines[index].start),
            None => pos,
        }
    }

    pub fn line_end(&self, pos: Pos) -> Pos {
        match self.line_for(pos) {
            Some(index) => Pos::new(pos.block, self.lines[index].start + self.line_max(index)),
            None => pos,
        }
    }
}
