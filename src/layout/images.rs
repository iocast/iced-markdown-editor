//! Images inside text: sizes, line heights and placement.

use super::{BlockMetrics, Decor, IMAGE_GAP, ImageLookup, Layout};
use crate::doc::Block;
use crate::images::LoadedImage;

pub(super) struct ImageRun {
    start: usize,
    end: usize,
    width: f32,
    height: f32,
    image: LoadedImage,
}

pub(super) type ImageSizes = Vec<Option<(f32, f32)>>;

pub(super) fn block_images(
    block: &Block,
    images: ImageLookup<'_>,
    zoom: f32,
    max_width: f32,
    max_height: f32,
) -> (ImageSizes, Vec<ImageRun>) {
    let len = block.chars.len();
    let mut sizes = vec![None; len];
    let mut runs = Vec::new();
    if block.leaf.is_plain() {
        return (sizes, runs);
    }
    let mut i = 0;
    while i < len {
        let Some(link) = block.styles[i].image.clone() else {
            i += 1;
            continue;
        };
        let mut j = i + 1;
        while j < len && block.styles[j].image.as_ref() == Some(&link) {
            j += 1;
        }
        if let Some(image) = images(&link.url) {
            let mut width = image.width * zoom;
            let mut height = image.height * zoom;
            let max_width = (max_width - 2.0).max(1.0);
            if width > max_width {
                height *= max_width / width;
                width = max_width;
            }
            let max_height = (max_height - 2.0 * IMAGE_GAP * zoom).max(1.0);
            if height > max_height {
                width *= max_height / height;
                height = max_height;
            }
            sizes[i] = Some((width, height));
            for size in &mut sizes[i + 1..j] {
                *size = Some((0.0, 0.0));
            }
            runs.push(ImageRun {
                start: i,
                end: j,
                width,
                height,
                image,
            });
        }
        i = j;
    }
    (sizes, runs)
}

/// Height of the visual line `start..end` and the height of its tallest image.
pub(super) fn row_height(
    sizes: &[Option<(f32, f32)>],
    start: usize,
    end: usize,
    m: &BlockMetrics,
    z: f32,
) -> (f32, f32) {
    let image_height = sizes[start..end]
        .iter()
        .flatten()
        .map(|(_, h)| *h)
        .fold(0.0, f32::max);
    let height = if image_height > 0.0 {
        m.line_height.max(image_height + 2.0 * IMAGE_GAP * z)
    } else {
        m.line_height
    };
    (height, image_height)
}

/// Images of the visual line `start..end` drawn at `x0` (line start) and
/// `y`; returns the character ranges they replace.
pub(super) fn place_images(
    layout: &mut Layout,
    runs: &[ImageRun],
    (start, end): (usize, usize),
    xs: &[f32],
    x0: f32,
    bottom: f32,
) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    for run in runs {
        if run.start >= start && run.start < end {
            layout.decor.push(Decor::Image {
                x: x0 + xs[run.start - start],
                y: bottom - run.height,
                w: run.width,
                h: run.height,
                image: run.image.clone(),
            });
        }
        let from = run.start.max(start);
        let to = run.end.min(end);
        if from < to {
            ranges.push((from, to));
        }
    }
    ranges
}
