//! Replays the core's display list onto an Iced canvas frame, skipping what
//! is not visible.

use crate::convert;
use crate::fonts;
use iced::widget::canvas::{self, Frame};
use iced::widget::text::{LineHeight, Shaping};
use iced::{Pixels, Vector};
use markdown_editor_core::draw::{Op, Path, PathSegment, TextRun, Transform};
use markdown_editor_core::geometry::{Point, Rectangle};

/// Replays `ops`, shifted by `offset` (the scroll position); `visible` is in
/// the coordinates of the operations.
pub(crate) fn replay(ops: &[Op], frame: &mut Frame, visible: Option<Rectangle>, offset: Vector) {
    frame.with_save(|frame| {
        frame.translate(offset);
        draw_all(ops, frame, visible, offset);
    });
}

fn draw_all(ops: &[Op], frame: &mut Frame, visible: Option<Rectangle>, offset: Vector) {
    for op in ops {
        if op.is_visible(visible) {
            draw(op, frame, visible, offset);
        }
    }
}

fn draw(op: &Op, frame: &mut Frame, visible: Option<Rectangle>, offset: Vector) {
    match op {
        Op::Rect { rect, color } => frame.fill_rectangle(
            convert::point(rect.position()),
            convert::size(rect.size()),
            convert::color(*color),
        ),
        Op::Fill { path, color, .. } => frame.fill(&iced_path(path), convert::color(*color)),
        Op::Stroke {
            path, width, color, ..
        } => frame.stroke(
            &iced_path(path),
            canvas::Stroke::default()
                .with_width(*width)
                .with_color(convert::color(*color)),
        ),
        Op::Text { run, .. } => frame.fill_text(text(run)),
        Op::Image { rect, image } => draw_image(frame, *rect, image),
        Op::Transformed { transform, op } => frame.with_save(|frame| {
            let (translation, rotation, scale) = transform.decompose();
            frame.translate(Vector::new(translation.x, translation.y));
            frame.rotate(rotation);
            frame.scale_nonuniform(Vector::new(scale.x, scale.y));
            draw(op, frame, None, offset);
        }),
        Op::Clip {
            rect,
            transform,
            ops,
        } => {
            // A clipped frame starts without the transforms of its parent.
            let region = clip_region(*rect, *transform, offset);
            frame.with_clip(region, |frame| {
                frame.translate(offset);
                draw_all(ops, frame, visible, offset);
            });
        }
        Op::Group { ops, .. } => draw_all(ops, frame, None, offset),
    }
}

/// The area of a clip on the frame: the box around the transformed rectangle.
fn clip_region(rect: Rectangle, transform: Transform, offset: Vector) -> iced::Rectangle {
    let corners = [
        Point::new(rect.x, rect.y),
        Point::new(rect.x + rect.width, rect.y),
        Point::new(rect.x, rect.y + rect.height),
        Point::new(rect.x + rect.width, rect.y + rect.height),
    ]
    .map(|corner| transform.apply(corner));
    let min = |value: fn(&Point) -> f32| corners.iter().map(value).fold(f32::INFINITY, f32::min);
    let max =
        |value: fn(&Point) -> f32| corners.iter().map(value).fold(f32::NEG_INFINITY, f32::max);
    let (left, top) = (min(|p| p.x), min(|p| p.y));
    convert::rectangle(Rectangle {
        x: left + offset.x,
        y: top + offset.y,
        width: max(|p| p.x) - left,
        height: max(|p| p.y) - top,
    })
}

fn iced_path(path: &Path) -> canvas::Path {
    canvas::Path::new(|builder| {
        for segment in path.segments() {
            match *segment {
                PathSegment::MoveTo(to) => builder.move_to(convert::point(to)),
                PathSegment::LineTo(to) => builder.line_to(convert::point(to)),
                PathSegment::QuadTo(control, to) => {
                    builder.quadratic_curve_to(convert::point(control), convert::point(to));
                }
                PathSegment::CubicTo(a, b, to) => builder.bezier_curve_to(
                    convert::point(a),
                    convert::point(b),
                    convert::point(to),
                ),
                PathSegment::Close => builder.close(),
            }
        }
    })
}

fn text(run: &TextRun) -> canvas::Text {
    canvas::Text {
        content: run.content.clone(),
        position: convert::point(run.origin),
        color: convert::color(run.color),
        size: Pixels(run.size),
        line_height: LineHeight::Absolute(Pixels(run.line_height)),
        font: fonts::font(run.face),
        // Basic shaping places the glyphs at the measured advances.
        shaping: Shaping::Basic,
        ..canvas::Text::default()
    }
}

#[cfg(feature = "images")]
fn draw_image(frame: &mut Frame, rect: Rectangle, image: &markdown_editor_core::draw::Image) {
    use iced::widget::image::Handle;
    use std::sync::Arc;

    /// The pixels shared with the core, without a copy.
    struct Pixels(Arc<Vec<u8>>);

    impl AsRef<[u8]> for Pixels {
        fn as_ref(&self) -> &[u8] {
            &self.0
        }
    }

    // Created once, so Iced keeps the texture cached.
    let handle = image.handle(|image| {
        let pixels = bytes::Bytes::from_owner(Pixels(image.pixels().clone()));
        Handle::from_rgba(image.width(), image.height(), pixels)
    });
    frame.draw_image(convert::rectangle(rect), canvas::Image::new(handle.clone()));
}

#[cfg(not(feature = "images"))]
fn draw_image(_: &mut Frame, _: Rectangle, _: &markdown_editor_core::draw::Image) {}

#[cfg(test)]
mod tests {
    use super::*;
    use markdown_editor_core::geometry::Size;

    #[test]
    fn clip_regions_follow_the_scroll_position() {
        let rect = Rectangle::new(Point::new(10.0, 20.0), Size::new(30.0, 40.0));
        let region = clip_region(rect, Transform::IDENTITY, Vector::new(-5.0, -15.0));
        assert_eq!(
            region,
            iced::Rectangle {
                x: 5.0,
                y: 5.0,
                width: 30.0,
                height: 40.0,
            }
        );
    }

    #[test]
    fn clip_regions_cover_transformed_rectangles() {
        let rect = Rectangle::new(Point::ORIGIN, Size::new(10.0, 10.0));
        let moved = Transform::translate(5.0, 5.0).then_local(Transform::scale(2.0, 2.0));
        let region = clip_region(rect, moved, Vector::new(0.0, 0.0));
        assert_eq!((region.x, region.y), (5.0, 5.0));
        assert_eq!((region.width, region.height), (20.0, 20.0));
    }
}
