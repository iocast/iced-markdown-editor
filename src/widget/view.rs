//! The widget: page canvas (with its own scrolling), toolbar, link bar,
//! context menu and status bar. All labels use the bundled fonts, so they render in the
//! browser too.

use super::toolbar::{self, fill};
use super::{Action, Command, EditorState, Inner, Mode};
use super::{dialog, menu};
use crate::convert;
use crate::draw;
use crate::fonts;
use crate::i18n;
use crate::render;
use iced::keyboard::{self, Key, Modifiers, key::Named};
use iced::mouse::{self, ScrollDelta};
use iced::widget::canvas::{self, Canvas, Frame, Geometry};
use iced::widget::{Stack, Text, button, column, container, row, text, text_input};
use iced::{
    Alignment, Background, Border, Color, Element, Length, Point, Rectangle, Renderer, Size, Theme,
    Vector, window,
};
use markdown_editor_core::draw::Frame as CoreFrame;
use markdown_editor_core::editor::builtin_command;
use std::time::Duration;
use web_time::Instant;

/// Width of the scrollbars.
const BAR: f32 = 10.0;
/// Pixels per wheel "line".
const LINE: f32 = 48.0;

/// A label in the bundled UI font.
fn label<'a>(content: impl Into<String>, size: u32) -> Text<'a> {
    let content: String = content.into();
    text(content).size(size).font(fonts::SANS)
}

/// What the left mouse button is doing.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
enum Drag {
    #[default]
    None,
    /// Selecting text.
    Select,
    /// Dragging the vertical scrollbar; offset of the grab in the thumb.
    Vertical(f32),
    /// Dragging the horizontal scrollbar.
    Horizontal(f32),
}

/// Interaction state kept by the canvas itself.
#[derive(Default)]
pub struct PageState {
    drag: Drag,
    last_click: Option<(Instant, Point)>,
    clicks: u8,
    modifiers: Modifiers,
}

/// Scrollbar geometry: track and thumb, in widget coordinates.
#[derive(Debug, Clone, Copy)]
struct Bar {
    track: Rectangle,
    thumb: Rectangle,
    /// Largest scroll offset.
    max: f32,
}

/// The vertical and horizontal scrollbar, when the content overflows.
fn bars(editor: &EditorState, size: Size) -> (Option<Bar>, Option<Bar>) {
    let (content_w, content_h) = {
        let layout = editor.layout.borrow();
        (layout.width, layout.height)
    };
    let (sx, sy) = editor.clamped_scroll();
    let need_v = content_h > size.height + 0.5;
    let need_h = content_w > size.width + 0.5;

    let vertical = need_v.then(|| {
        let length = size.height - if need_h { BAR } else { 0.0 };
        let max = (content_h - size.height).max(1.0);
        let thumb = (length * size.height / content_h).clamp(24.0, length);
        let y = (sy / max) * (length - thumb);
        Bar {
            track: Rectangle::new(Point::new(size.width - BAR, 0.0), Size::new(BAR, length)),
            thumb: Rectangle::new(Point::new(size.width - BAR, y), Size::new(BAR, thumb)),
            max,
        }
    });
    let horizontal = need_h.then(|| {
        let length = size.width - if need_v { BAR } else { 0.0 };
        let max = (content_w - size.width).max(1.0);
        let thumb = (length * size.width / content_w).clamp(24.0, length);
        let x = (sx / max) * (length - thumb);
        Bar {
            track: Rectangle::new(Point::new(0.0, size.height - BAR), Size::new(length, BAR)),
            thumb: Rectangle::new(Point::new(x, size.height - BAR), Size::new(thumb, BAR)),
            max,
        }
    });
    (vertical, horizontal)
}

struct Page<'a> {
    state: &'a EditorState,
    /// Right press opens the context menu.
    context_menu: bool,
    /// As tall as the content (`height(Length::Shrink)`).
    fit: bool,
}

fn publish(action: Inner) -> Option<canvas::Action<Action>> {
    Some(canvas::Action::publish(Action(action)).and_capture())
}

impl<'a> canvas::Program<Action> for Page<'a> {
    type State = PageState;

    fn update(
        &self,
        page: &mut PageState,
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Action>> {
        let editor = self.state;
        let size = bounds.size();
        let (sx, sy) = editor.clamped_scroll();

        match event {
            // As tall as the content: the height is set in `view`, from the
            // layout for the last width. A new width changes the height;
            // an empty scroll is the message that makes the application
            // build the view again.
            canvas::Event::Window(window::Event::RedrawRequested(_)) if self.fit => {
                editor.sync_viewport(convert::size_from_iced(size));
                let content = editor.layout.borrow().content_height;
                ((content - size.height).abs() > 0.5)
                    .then(|| canvas::Action::publish(Action(Inner::ScrollBy { dx: 0.0, dy: 0.0 })))
            }
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let Some(point) = cursor.position_in(bounds) else {
                    return editor
                        .focused
                        .then(|| canvas::Action::publish(Action(Inner::PressOutside)));
                };

                let (vertical, horizontal) = bars(editor, size);
                if let Some(bar) = vertical.filter(|bar| bar.track.contains(point)) {
                    let grab = if bar.thumb.contains(point) {
                        point.y - bar.thumb.y
                    } else {
                        bar.thumb.height / 2.0
                    };
                    page.drag = Drag::Vertical(grab);
                    let y = scroll_for(
                        point.y - grab,
                        bar.track.y,
                        bar.track.height,
                        bar.thumb.height,
                        bar.max,
                    );
                    return publish(Inner::ScrollTo {
                        x: None,
                        y: Some(y),
                    });
                }
                if let Some(bar) = horizontal.filter(|bar| bar.track.contains(point)) {
                    let grab = if bar.thumb.contains(point) {
                        point.x - bar.thumb.x
                    } else {
                        bar.thumb.width / 2.0
                    };
                    page.drag = Drag::Horizontal(grab);
                    let x = scroll_for(
                        point.x - grab,
                        bar.track.x,
                        bar.track.width,
                        bar.thumb.width,
                        bar.max,
                    );
                    return publish(Inner::ScrollTo {
                        x: Some(x),
                        y: None,
                    });
                }

                let now = Instant::now();
                let clicks = match page.last_click {
                    Some((time, previous))
                        if now.duration_since(time) < Duration::from_millis(450)
                            && (previous.x - point.x).abs() < 5.0
                            && (previous.y - point.y).abs() < 5.0 =>
                    {
                        page.clicks % 3 + 1
                    }
                    _ => 1,
                };
                page.clicks = clicks;
                page.last_click = Some((now, point));
                let (x, y) = (point.x + sx, point.y + sy);
                page.drag = if editor.layout.borrow().task_at(x, y).is_some() {
                    Drag::None
                } else {
                    Drag::Select
                };
                publish(Inner::Press { x, y, clicks })
            }
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right))
                if self.context_menu && page.drag == Drag::None =>
            {
                let point = cursor.position_in(bounds)?;
                let (vertical, horizontal) = bars(editor, size);
                if vertical.is_some_and(|bar| bar.track.contains(point))
                    || horizontal.is_some_and(|bar| bar.track.contains(point))
                {
                    return None;
                }
                publish(Inner::ContextMenu {
                    at: convert::point_from_iced(point),
                    x: point.x + sx,
                    y: point.y + sy,
                })
            }
            canvas::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                let position = cursor.position()?;
                let point = Point::new(position.x - bounds.x, position.y - bounds.y);
                match page.drag {
                    Drag::None => None,
                    Drag::Select => publish(Inner::Drag {
                        x: point.x + sx,
                        y: point.y + sy,
                    }),
                    Drag::Vertical(grab) => {
                        let bar = bars(editor, size).0?;
                        let y = scroll_for(
                            point.y - grab,
                            bar.track.y,
                            bar.track.height,
                            bar.thumb.height,
                            bar.max,
                        );
                        publish(Inner::ScrollTo {
                            x: None,
                            y: Some(y),
                        })
                    }
                    Drag::Horizontal(grab) => {
                        let bar = bars(editor, size).1?;
                        let x = scroll_for(
                            point.x - grab,
                            bar.track.x,
                            bar.track.width,
                            bar.thumb.width,
                            bar.max,
                        );
                        publish(Inner::ScrollTo {
                            x: Some(x),
                            y: None,
                        })
                    }
                }
            }
            canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                if page.drag != Drag::None {
                    page.drag = Drag::None;
                    return Some(canvas::Action::capture());
                }
                None
            }
            canvas::Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                if !cursor.is_over(bounds) {
                    return None;
                }
                let settings = &editor.zoom_settings;
                if page.modifiers.command() && settings.enabled && settings.mouse_wheel {
                    return publish(Inner::Wheel(convert::scroll_delta(*delta)));
                }
                let (mut dx, mut dy) = match *delta {
                    ScrollDelta::Lines { x, y } => (-x * LINE, -y * LINE),
                    ScrollDelta::Pixels { x, y } => (-x, -y),
                };
                if page.modifiers.shift() && dx == 0.0 {
                    std::mem::swap(&mut dx, &mut dy);
                }
                // Content that fits leaves the wheel to a scrolling parent.
                let (vertical, horizontal) = bars(editor, size);
                let dx = if horizontal.is_some() { dx } else { 0.0 };
                let dy = if vertical.is_some() { dy } else { 0.0 };
                if dx == 0.0 && dy == 0.0 {
                    return None;
                }
                publish(Inner::ScrollBy { dx, dy })
            }
            canvas::Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) => {
                page.modifiers = *modifiers;
                Some(canvas::Action::publish(Action(Inner::Modifiers(
                    convert::modifiers(*modifiers),
                ))))
            }
            canvas::Event::Keyboard(keyboard::Event::KeyPressed {
                key,
                modified_key,
                modifiers,
                text,
                ..
            }) => {
                if editor.link_url.is_some() {
                    return None;
                }
                let modifiers = *modifiers;

                if editor.focused || cursor.is_over(bounds) {
                    if let Some(command) = editor.zoom_settings.command_for(
                        &convert::key(key),
                        &convert::key(modified_key),
                        convert::modifiers(modifiers),
                    ) {
                        return Some(canvas::Action::publish(Action::from(command)).and_capture());
                    }
                }

                if !editor.focused {
                    return None;
                }

                if modifiers.command() && !modifiers.alt() {
                    if let Key::Character(c) = key.as_ref() {
                        let c = c.to_lowercase();
                        // Shortcuts of disabled features (or edits of a
                        // read-only document) go to the application.
                        builtin_command(&c, modifiers.shift())
                            .filter(|command| editor.allows(command))?;
                    }
                }

                let writable = !editor.read_only;
                let tab = editor.tab_width > 0 && !modifiers.command() && writable;
                let editing_key = match key.as_ref() {
                    Key::Named(Named::Tab) => tab,
                    Key::Named(
                        Named::ArrowLeft
                        | Named::ArrowRight
                        | Named::ArrowUp
                        | Named::ArrowDown
                        | Named::Home
                        | Named::End
                        | Named::PageUp
                        | Named::PageDown
                        | Named::Escape,
                    ) => true,
                    Key::Named(Named::Backspace | Named::Delete | Named::Enter) => writable,
                    // Ctrl/Cmd shortcuts were checked above.
                    Key::Character(_) => writable || modifiers.command(),
                    _ => false,
                };
                let types_text = writable
                    && text.as_ref().is_some_and(|text| {
                        !text.is_empty() && text.chars().all(|c| !c.is_control())
                    });
                if !editing_key && !types_text {
                    return None;
                }

                let text = text.as_ref().map(|text| text.to_string());
                publish(Inner::Key(
                    convert::key(key),
                    convert::modifiers(modifiers),
                    text,
                ))
            }
            _ => None,
        }
    }

    fn draw(
        &self,
        page: &PageState,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let editor = self.state;
        let size = bounds.size();
        let core_size = convert::size_from_iced(size);
        editor.sync_viewport(core_size);
        let (sx, sy) = editor.clamped_scroll();
        let palette = convert::palette(&editor.style.palette);
        let core_palette = editor.style.palette;
        let scroll = Vector::new(-sx, -sy);
        let visible = crate::Rectangle::new(crate::Point::new(sx, sy), core_size);
        let new_frame = || CoreFrame::new(core_size, fonts::shared().clone());

        // The content is drawn again when the layout or the scroll position
        // changes.
        let drawn = (editor.layout_generation.get(), sx.to_bits(), sy.to_bits());
        if editor.drawn.replace(Some(drawn)) != Some(drawn) {
            editor.cache.borrow_mut().clear();
        }
        let scene = editor.content_scene();
        let content = {
            let layout = editor.layout.borrow();
            editor.cache.borrow().draw(renderer, size, |frame| {
                let mut desk = new_frame();
                render::draw_desk(&mut desk, &layout, &core_palette, core_size);
                draw::replay(desk.ops(), frame, None, Vector::ZERO);
                draw::replay(&scene, frame, Some(visible), scroll);
            })
        };

        let mut overlay = Frame::new(renderer, size);
        {
            let layout = editor.layout.borrow();
            let mut marks = new_frame();
            render::draw_selection(&mut marks, &layout, editor.sel, &core_palette);
            if editor.focused && editor.caret_on && editor.link_url.is_none() && !editor.read_only {
                render::draw_caret(&mut marks, &layout, editor.sel, &core_palette);
            }
            draw::replay(marks.ops(), &mut overlay, Some(visible), scroll);
        }

        let hovered = cursor.position_in(bounds);
        let (vertical, horizontal) = bars(editor, size);
        for (bar, dragging) in [
            (vertical, matches!(page.drag, Drag::Vertical(_))),
            (horizontal, matches!(page.drag, Drag::Horizontal(_))),
        ] {
            let Some(bar) = bar else {
                continue;
            };
            let active = dragging || hovered.is_some_and(|point| bar.track.contains(point));
            let alpha = if active { 0.55 } else { 0.30 };
            let thumb = Color {
                a: alpha,
                ..palette.muted
            };
            let inset = 2.0;
            overlay.fill_rectangle(
                Point::new(bar.thumb.x + inset, bar.thumb.y + inset),
                Size::new(
                    bar.thumb.width - 2.0 * inset,
                    bar.thumb.height - 2.0 * inset,
                ),
                thumb,
            );
        }

        vec![content, overlay.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        page: &PageState,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if matches!(page.drag, Drag::Vertical(_) | Drag::Horizontal(_)) {
            return mouse::Interaction::Idle;
        }
        let Some(point) = cursor.position_in(bounds) else {
            return mouse::Interaction::default();
        };
        let (vertical, horizontal) = bars(self.state, bounds.size());
        if vertical.is_some_and(|bar| bar.track.contains(point))
            || horizontal.is_some_and(|bar| bar.track.contains(point))
        {
            return mouse::Interaction::Idle;
        }
        let (sx, sy) = self.state.clamped_scroll();
        // The checkboxes of a read-only document do not react.
        if !self.state.read_only
            && self
                .state
                .layout
                .borrow()
                .task_at(point.x + sx, point.y + sy)
                .is_some()
        {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::Text
        }
    }
}

/// Scroll offset for a thumb start position.
fn scroll_for(thumb_start: f32, track_start: f32, track: f32, thumb: f32, max: f32) -> f32 {
    let room = (track - thumb).max(1.0);
    ((thumb_start - track_start) / room).clamp(0.0, 1.0) * max
}

/// Builder returned by [`markdown_editor`].
pub struct MarkdownEditor<'a> {
    state: &'a EditorState,
    toolbar: bool,
    status_bar: bool,
    context_menu: bool,
    width: Length,
    height: Length,
}

/// Creates the editor widget for the given state.
pub fn markdown_editor(state: &EditorState) -> MarkdownEditor<'_> {
    MarkdownEditor {
        state,
        toolbar: true,
        status_bar: true,
        context_menu: true,
        width: Length::Fill,
        height: Length::Fill,
    }
}

impl<'a> MarkdownEditor<'a> {
    /// Shows or hides the built-in toolbar (default: shown).
    pub fn toolbar(mut self, show: bool) -> Self {
        self.toolbar = show;
        self
    }

    /// Shows or hides the status bar (default: shown).
    pub fn status_bar(mut self, show: bool) -> Self {
        self.status_bar = show;
        self
    }

    /// Enables or disables the context menu on the right mouse button
    /// (default: enabled).
    pub fn context_menu(mut self, enabled: bool) -> Self {
        self.context_menu = enabled;
        self
    }

    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    /// `Length::Shrink` makes the editor as tall as the document and the
    /// bars, for a scrolling parent: the page itself then never scrolls
    /// vertically (e.g. to show Markdown in a page, see
    /// [`EditorState::read_only`]).
    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = height.into();
        self
    }

    /// Builds the widget element.
    pub fn view(self) -> Element<'a, Action> {
        let state = self.state;
        let palette = convert::palette(&state.style.palette);

        let bar_style = move |_: &Theme| container::Style {
            background: Some(Background::Color(palette.toolbar_background)),
            text_color: Some(palette.toolbar_text),
            border: Border {
                color: palette.toolbar_border,
                width: 1.0,
                radius: 0.0.into(),
            },
            ..container::Style::default()
        };

        let language = state.language;
        let tr = |id| i18n::text(language, id);
        let mut content = column![];

        // Nothing on the toolbar works in a read-only document.
        if self.toolbar && !state.read_only {
            content = content.push(
                container(toolbar::view(state))
                    .padding([4, 8])
                    .width(Length::Fill)
                    .style(bar_style),
            );
        }

        if let Some(url) = state
            .link_url
            .as_ref()
            .filter(|_| state.link_dialog.is_none())
        {
            let link_bar = container(
                row![
                    label(tr("link-address"), 12).color(palette.muted),
                    text_input("https://", url)
                        .on_input(|url| Action(Inner::LinkUrl(url)))
                        .on_submit(Action(Inner::LinkApply))
                        .font(fonts::SANS)
                        .padding(6)
                        .size(13)
                        .width(Length::Fixed(420.0)),
                    button(label(tr("link-apply"), 12))
                        .on_press(Action(Inner::LinkApply))
                        .padding([5, 10])
                        .style(toolbar::button_style(palette, true)),
                    button(label(tr("link-remove"), 12))
                        .on_press(Action(Inner::LinkRemove))
                        .padding([5, 10])
                        .style(toolbar::button_style(palette, false)),
                    button(label(tr("link-cancel"), 12))
                        .on_press(Action(Inner::LinkCancel))
                        .padding([5, 10])
                        .style(toolbar::button_style(palette, false)),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            )
            .padding([6, 12])
            .width(Length::Fill)
            .style(bar_style);

            content = content.push(link_bar);
        }

        // `Shrink`: as tall as the document (and the bars), for a scrolling
        // parent; the page then never scrolls vertically.
        let fit = self.height == Length::Shrink;
        let (page_height, height) = if fit {
            let content = state.layout.borrow().content_height;
            (Length::Fixed(content), Length::Shrink)
        } else {
            (Length::Fill, Length::Fill)
        };
        let page = Canvas::new(Page {
            state,
            context_menu: self.context_menu,
            fit,
        })
        .width(Length::Fill)
        .height(page_height);
        // Always a stack, so the canvas keeps its state when the menu opens.
        let mut layers = Stack::new().width(Length::Fill).height(height).push(page);
        if let Some(at) = state.context_menu.filter(|_| self.context_menu) {
            layers = layers.push(menu::view(state, convert::point(at)));
        }
        content = content.push(layers);

        if self.status_bar {
            let mode = match state.mode {
                Mode::Wysiwyg => tr("status-mode-wysiwyg"),
                Mode::Source => tr("status-mode-source"),
            };

            let mut status = row![].spacing(18).align_y(Alignment::Center);

            if !state.paper.is_fill() {
                let page = i18n::text_with(
                    language,
                    "status-page",
                    &[
                        ("current", state.current_page().into()),
                        ("total", state.page_count().into()),
                    ],
                );
                status = status.push(label(page, 12));
            }

            let words = i18n::text_with(
                language,
                "status-words",
                &[("count", state.word_count().into())],
            );
            status = status.push(label(words, 12));

            let (checked, total) = state.task_progress();
            if total > 0 {
                let tasks = i18n::text_with(
                    language,
                    "status-tasks",
                    &[("checked", checked.into()), ("total", total.into())],
                );
                status = status.push(label(tasks, 12));
            }

            let pending = state.pending_images();
            if pending > 0 {
                let saving = i18n::text_with(
                    language,
                    "status-saving-images",
                    &[("count", pending.into())],
                );
                status = status.push(label(saving, 12));
            }

            status = status.push(label(mode, 12));

            if state.tab_width > 0 {
                let tab = i18n::text_with(
                    language,
                    "status-tab-width",
                    &[("count", state.tab_width.into())],
                );
                status = status.push(label(tab, 12));
            }

            status = status.push(fill()).push(label(
                if state.dirty {
                    tr("status-modified")
                } else {
                    String::new()
                },
                12,
            ));

            let zoom = &state.zoom_settings;
            if zoom.enabled && zoom.controls {
                let percent = format!("{:.0} %", state.zoom * 100.0);
                let small = |text: String, command: Command| {
                    button(label(text, 12))
                        .on_press(Action::from(command))
                        .padding([1, 8])
                        .style(toolbar::button_style(palette, false))
                };
                status = status.push(
                    row![
                        small("\u{2212}".to_string(), Command::ZoomOut),
                        small(percent, Command::ResetZoom),
                        small("+".to_string(), Command::ZoomIn),
                    ]
                    .spacing(2)
                    .align_y(Alignment::Center),
                );
            }

            content = content.push(
                container(status)
                    .padding([3, 12])
                    .width(Length::Fill)
                    .style(move |_: &Theme| container::Style {
                        background: Some(Background::Color(palette.toolbar_background)),
                        text_color: Some(palette.muted),
                        ..container::Style::default()
                    }),
            );
        }

        // Always a stack, so the editor keeps its state when the dialog opens.
        let mut editor = Stack::new()
            .width(Length::Fill)
            .height(height)
            .push(content.width(Length::Fill).height(height));
        if let (Some(link), Some(url)) = (&state.link_dialog, &state.link_url) {
            for layer in dialog::view(state, link, url) {
                editor = editor.push(layer);
            }
        }

        container(editor)
            .width(self.width)
            .height(self.height)
            .into()
    }
}

impl<'a> From<MarkdownEditor<'a>> for Element<'a, Action> {
    fn from(editor: MarkdownEditor<'a>) -> Self {
        editor.view()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::page::Paper;
    use crate::widget::Event;
    use iced::event::Status;
    use iced::keyboard::Location;
    use iced::keyboard::key::{NativeCode, Physical};
    use markdown_editor_core::doc::Pos;

    const BOUNDS: Rectangle = Rectangle {
        x: 0.0,
        y: 0.0,
        width: 600.0,
        height: 400.0,
    };

    /// A page canvas with its own interaction state, as Iced keeps it.
    struct Harness {
        editor: EditorState,
        page: PageState,
        bounds: Rectangle,
        fit: bool,
        events: Vec<Event>,
    }

    impl Harness {
        fn new(markdown: &str, read_only: bool) -> Harness {
            let editor = EditorState::with_markdown(markdown)
                .paper(Paper::fill())
                .read_only(read_only);
            editor.sync_viewport(convert::size_from_iced(BOUNDS.size()));
            Harness {
                editor,
                page: PageState::default(),
                bounds: BOUNDS,
                fit: false,
                events: Vec::new(),
            }
        }

        /// Feeds `event` to the canvas and the published action to the
        /// editor, like an application. Returns whether the canvas took the
        /// event (captured it).
        fn send(&mut self, event: canvas::Event, at: Point) -> bool {
            let program = Page {
                state: &self.editor,
                context_menu: true,
                fit: self.fit,
            };
            let action = canvas::Program::update(
                &program,
                &mut self.page,
                &event,
                self.bounds,
                mouse::Cursor::Available(at),
            );
            let Some(action) = action else {
                return false;
            };
            let (message, _, status) = action.into_inner();
            if let Some(message) = message {
                let output = self.editor.update(message);
                self.events.extend(output.events);
            }
            status == Status::Captured
        }

        /// Inside the character at `offset` of block `block`.
        fn at(&self, block: usize, offset: usize) -> Point {
            let (x, y, height) = self
                .editor
                .layout
                .borrow()
                .caret(Pos::new(block, offset))
                .expect("a caret position");
            Point::new(x + 1.0, y + height / 2.0)
        }

        fn click(&mut self, at: Point) {
            self.send(
                canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
                at,
            );
            self.send(
                canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
                at,
            );
        }

        /// A key press; returns whether the editor took it.
        fn key(&mut self, key: Key, modifiers: Modifiers, text: Option<&str>) -> bool {
            let event = canvas::Event::Keyboard(keyboard::Event::KeyPressed {
                key: key.clone(),
                modified_key: key,
                physical_key: Physical::Unidentified(NativeCode::Unidentified),
                location: Location::Standard,
                modifiers,
                text: text.map(Into::into),
                repeat: false,
            });
            self.send(event, Point::new(10.0, 10.0))
        }

        fn typed(&mut self, c: &str) -> bool {
            self.key(Key::Character(c.into()), Modifiers::empty(), Some(c))
        }

        fn shortcut(&mut self, c: &str) -> bool {
            self.key(Key::Character(c.into()), Modifiers::COMMAND, None)
        }

        fn named(&mut self, key: Named) -> bool {
            self.key(Key::Named(key), Modifiers::empty(), None)
        }

        fn wheel(&mut self, lines: f32) -> bool {
            let delta = ScrollDelta::Lines { x: 0.0, y: lines };
            self.send(
                canvas::Event::Mouse(mouse::Event::WheelScrolled { delta }),
                Point::new(10.0, 10.0),
            )
        }
    }

    #[test]
    fn typing_edits_an_editable_document() {
        let mut h = Harness::new("hello\n", false);
        let at = h.at(0, 5);
        h.click(at);
        assert!(h.typed("s"));
        assert_eq!(h.editor.markdown(), "hellos\n");
    }

    #[test]
    fn a_read_only_document_leaves_keys_to_the_application() {
        let text = "hello world\n";
        let mut h = Harness::new(text, true);
        let at = h.at(0, 2);
        h.click(at);
        assert!(h.editor.is_focused(), "it can still be focused");

        assert!(!h.typed("x"));
        assert!(!h.named(Named::Backspace));
        assert!(!h.named(Named::Delete));
        assert!(!h.named(Named::Enter));
        assert!(!h.named(Named::Tab));
        assert!(!h.shortcut("b"));
        assert!(!h.shortcut("x"));
        assert!(!h.shortcut("v"));
        assert!(!h.shortcut("z"));
        assert_eq!(h.editor.markdown(), text);
        assert!(!h.events.contains(&Event::Changed));

        // Moving, selecting and copying still work.
        assert!(h.named(Named::ArrowRight));
        assert!(h.shortcut("a"));
        assert_eq!(h.editor.selected_text().as_deref(), Some("hello world"));
        assert!(h.shortcut("c"));
    }

    #[test]
    fn a_read_only_document_follows_links_with_a_click() {
        let mut h = Harness::new("[the site](https://example.org)\n", true);
        let at = h.at(0, 3);
        h.click(at);
        assert!(
            h.events
                .contains(&Event::LinkClicked("https://example.org".into()))
        );
    }

    #[test]
    fn the_wheel_goes_to_the_parent_when_nothing_scrolls() {
        let mut h = Harness::new("one line\n", false);
        assert!(!h.wheel(-3.0));

        let long: String = (0..200).map(|i| format!("line {i}\n\n")).collect();
        let mut h = Harness::new(&long, false);
        assert!(h.wheel(-3.0));
        assert!((h.editor.scroll_y - 3.0 * LINE).abs() < 0.5);
    }

    #[test]
    fn a_fitted_page_asks_for_its_content_height() {
        let mut h = Harness::new("one line\n", true);
        h.fit = true;
        let redraw =
            || canvas::Event::Window(window::Event::RedrawRequested(iced::time::Instant::now()));
        // Laid out at the old height: a message, so the view is built again.
        h.send(redraw(), Point::ORIGIN);
        assert_eq!(h.editor.scroll_y, 0.0, "the message changes nothing");
        let content = h.editor.layout.borrow().content_height;
        assert!(content > 0.0 && content < 100.0, "{content}");
        fn asks(h: &Harness) -> bool {
            let program = Page {
                state: &h.editor,
                context_menu: true,
                fit: true,
            };
            let redraw =
                canvas::Event::Window(window::Event::RedrawRequested(iced::time::Instant::now()));
            canvas::Program::update(
                &program,
                &mut PageState::default(),
                &redraw,
                h.bounds,
                mouse::Cursor::Unavailable,
            )
            .is_some()
        }
        assert!(asks(&h));

        // At the content height: nothing to do.
        h.bounds.height = content;
        assert!(!asks(&h));

        // A narrower page wraps into more lines: taller again.
        h.editor.set_markdown(&"word ".repeat(200));
        h.bounds.width = 200.0;
        assert!(asks(&h));
        assert!(h.editor.layout.borrow().content_height > content * 3.0);
    }
}
