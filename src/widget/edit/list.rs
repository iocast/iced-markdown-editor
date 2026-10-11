//! Lists, task lists and block quotes: container operations on the document.

use super::super::{Action, EditKind, EditorState};
use crate::doc::{Block, Container, Document, Leaf, ListInfo};
use iced::Task;

/// Depth of the innermost container when it is a list item.
pub(super) fn item_depth(block: &Block) -> Option<usize> {
    match block.containers.last() {
        Some(Container::Item { .. }) => Some(block.containers.len() - 1),
        _ => None,
    }
}

pub(super) fn quote_depth(block: &Block) -> Option<usize> {
    match block.containers.last() {
        Some(Container::Quote { .. }) => Some(block.containers.len() - 1),
        _ => None,
    }
}

fn same_path(a: &[Container], b: &[Container]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.same(y))
}

/// Moves the list item containing `index` one level deeper, into the
/// previous item. Returns `false` when there is no previous item.
pub(super) fn nest_item(doc: &mut Document, index: usize) -> bool {
    let Some(depth) = item_depth(&doc.blocks[index]) else {
        return false;
    };
    let start = doc.container_start(index, depth);
    if start == 0 {
        return false;
    }
    let block = &doc.blocks[start];
    let Container::Item { list, task, .. } = block.containers[depth] else {
        return false;
    };
    let previous = &doc.blocks[start - 1];
    if previous.containers.len() <= depth
        || !(0..depth).all(|k| previous.containers[k].same(&block.containers[k]))
    {
        return false;
    }
    let parent = previous.containers[depth];
    let Container::Item {
        list: parent_list, ..
    } = parent
    else {
        return false;
    };
    if parent_list.id != list.id {
        return false;
    }
    let nested = match previous.containers.get(depth + 1) {
        Some(Container::Item { list: nested, .. }) => *nested,
        _ => ListInfo::new(list.ordered, 1),
    };
    let item = Container::task_item(nested, task);
    let end = doc.container_end(start, depth);
    for block in &mut doc.blocks[start..end] {
        block.containers[depth] = parent;
        block.containers.insert(depth + 1, item);
    }
    true
}

/// Moves the list item containing `index` one level out. A top-level item
/// becomes normal paragraphs.
pub(super) fn unnest_item(doc: &mut Document, index: usize) -> bool {
    let Some(depth) = item_depth(&doc.blocks[index]) else {
        return false;
    };
    let start = doc.container_start(index, depth);
    let end = doc.container_end(start, depth);
    let task = doc.blocks[start].containers[depth].task();
    let outer = depth
        .checked_sub(1)
        .map(|d| doc.blocks[start].containers[d])
        .filter(|c| matches!(c, Container::Item { .. }));

    match outer {
        Some(outer @ Container::Item { list, .. }) => {
            let new_outer = Container::task_item(list, task);
            for block in &mut doc.blocks[start..end] {
                block.containers.remove(depth);
                block.containers[depth - 1] = new_outer;
            }
            let mut k = end;
            while k < doc.blocks.len()
                && doc.blocks[k]
                    .containers
                    .get(depth - 1)
                    .is_some_and(|c| c.same(&outer))
            {
                doc.blocks[k].containers[depth - 1] = new_outer;
                k += 1;
            }
        }
        _ => {
            for block in &mut doc.blocks[start..end] {
                block.containers.remove(depth);
            }
        }
    }
    true
}

/// Puts the blocks `first..=last` into a list (each block one item).
pub(super) fn wrap_list(
    doc: &mut Document,
    first: usize,
    last: usize,
    ordered: bool,
    start: u64,
    task: Option<bool>,
) {
    let mut index = first;
    while index <= last {
        let base = doc.blocks[index].containers.clone();
        let mut end = index;
        while end < last && same_path(&doc.blocks[end + 1].containers, &base) {
            end += 1;
        }
        let join = index
            .checked_sub(1)
            .map(|p| &doc.blocks[p])
            .filter(|p| {
                p.containers.len() > base.len() && same_path(&p.containers[..base.len()], &base)
            })
            .and_then(|p| p.containers[base.len()].list())
            .filter(|list| list.ordered == ordered);
        let list = join.unwrap_or_else(|| ListInfo::new(ordered, start));
        for block in &mut doc.blocks[index..=end] {
            if block.leaf == Leaf::Rule {
                continue;
            }
            block.containers.push(Container::task_item(list, task));
        }
        index = end + 1;
    }
}

/// Puts the blocks `first..=last` into a block quote.
pub(super) fn wrap_quote(doc: &mut Document, first: usize, last: usize) {
    let mut index = first;
    while index <= last {
        let base = doc.blocks[index].containers.clone();
        let mut end = index;
        while end < last && same_path(&doc.blocks[end + 1].containers, &base) {
            end += 1;
        }
        let join = index
            .checked_sub(1)
            .map(|p| &doc.blocks[p])
            .filter(|p| {
                p.containers.len() > base.len() && same_path(&p.containers[..base.len()], &base)
            })
            .map(|p| p.containers[base.len()])
            .filter(Container::is_quote);
        let quote = join.unwrap_or_else(Container::quote);
        for block in &mut doc.blocks[index..=end] {
            block.containers.push(quote);
        }
        index = end + 1;
    }
}

impl EditorState {
    pub(super) fn toggle_checked(&mut self) -> Task<Action> {
        let (a, b) = self.sel.range();
        let tasks: Vec<(usize, usize, bool)> = (a.block..=b.block)
            .filter_map(|index| {
                let block = &self.doc.blocks[index];
                let depth = block.containers.iter().rposition(|c| c.task().is_some())?;
                Some((index, depth, block.containers[depth].task()?))
            })
            .collect();
        if tasks.is_empty() {
            return Task::none();
        }
        let value = tasks.iter().any(|(_, _, checked)| !checked);
        self.begin_edit(EditKind::Other);
        for (index, depth, _) in tasks {
            self.doc.set_task(index, depth, Some(value));
        }
        self.after_edit()
    }
}
