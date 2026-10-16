//! The widget: page canvas (with its own scrolling), toolbar, link bar and
//! status bar. All labels use the bundled fonts, so they render in the
//! browser too.

use super::edit::builtin_command;
use super::toolbar::{self, fill};
use super::{Action, Command, EditorState, Inner, Mode};
use crate::fonts;
use crate::i18n;
use crate::render;
use iced::keyboard::{self, Key, Modifiers, key::Named};
use iced::mouse::{self, ScrollDelta};
use iced::widget::canvas::{self, Canvas, Frame, Geometry};
use iced::widget::{Text, button, column, container, row, text, text_input};
use iced::{
    Alignment, Background, Border, Color, Element, Length, Point, Rectangle, Renderer, Size, Theme,
    Vector,
};
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
                    return publish(Inner::Wheel(*delta));
                }
                let (mut dx, mut dy) = match *delta {
                    ScrollDelta::Lines { x, y } => (-x * LINE, -y * LINE),
                    ScrollDelta::Pixels { x, y } => (-x, -y),
                };
                if page.modifiers.shift() && dx == 0.0 {
                    std::mem::swap(&mut dx, &mut dy);
                }
                publish(Inner::ScrollBy { dx, dy })
            }
            canvas::Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) => {
                page.modifiers = *modifiers;
                Some(canvas::Action::publish(Action(Inner::Modifiers(
                    *modifiers,
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
                    if let Some(command) =
                        editor
                            .zoom_settings
                            .command_for(key, modified_key, modifiers)
                    {
                        return Some(canvas::Action::publish(Action::from(command)).and_capture());
                    }
                }

                if !editor.focused {
                    return None;
                }

                if modifiers.command() && !modifiers.alt() {
                    if let Key::Character(c) = key.as_ref() {
                        let c = c.to_lowercase();
                        // Shortcuts of disabled features go to the application.
                        builtin_command(&c, modifiers.shift())
                            .filter(|command| editor.features.allows(command))?;
                    }
                }

                let tab = editor.tab_width > 0 && !modifiers.command();
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
                        | Named::Backspace
                        | Named::Delete
                        | Named::Enter
                        | Named::Escape,
                    ) => true,
                    Key::Character(_) => true,
                    _ => false,
                };
                let types_text = text
                    .as_ref()
                    .is_some_and(|text| !text.is_empty() && text.chars().all(|c| !c.is_control()));
                if !editing_key && !types_text {
                    return None;
                }

                let text = text.as_ref().map(|text| text.to_string());
                publish(Inner::Key(key.clone(), modifiers, text))
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
        editor.sync_viewport(size);
        let (sx, sy) = editor.clamped_scroll();
        let palette = editor.style.palette;
        let code_colors = |index: usize| editor.code_colors(index);
        let custom = |frame: &mut Frame, index: usize, bounds: Rectangle, available: Size| {
            editor.draw_block(frame, index, bounds, available)
        };

        let content = {
            let layout = editor.layout.borrow();
            editor.cache.borrow().draw(renderer, size, |frame| {
                render::draw_desk(frame, &layout, &palette, size);
                frame.translate(Vector::new(-sx, -sy));
                render::draw_pages(frame, &layout, &palette);
                render::draw_document(
                    frame,
                    &editor.doc,
                    &layout,
                    &palette,
                    &code_colors,
                    &custom,
                    (sy, sy + size.height),
                );
            })
        };

        let mut overlay = Frame::new(renderer, size);
        {
            let layout = editor.layout.borrow();
            overlay.with_save(|frame| {
                frame.translate(Vector::new(-sx, -sy));
                render::draw_selection(frame, &layout, editor.sel, &palette);
                if editor.focused && editor.caret_on && editor.link_url.is_none() {
                    render::draw_caret(frame, &layout, editor.sel, &palette);
                }
            });
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
        if self
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
    width: Length,
    height: Length,
}

/// Creates the editor widget for the given state.
pub fn markdown_editor(state: &EditorState) -> MarkdownEditor<'_> {
    MarkdownEditor {
        state,
        toolbar: true,
        status_bar: true,
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

    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = height.into();
        self
    }

    /// Builds the widget element.
    pub fn view(self) -> Element<'a, Action> {
        let state = self.state;
        let palette = state.style.palette;

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

        if self.toolbar {
            content = content.push(
                container(toolbar::view(state))
                    .padding([4, 8])
                    .width(Length::Fill)
                    .style(bar_style),
            );
        }

        if let Some(url) = &state.link_url {
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

        let page = Canvas::new(Page { state })
            .width(Length::Fill)
            .height(Length::Fill);
        content = content.push(page);

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

        container(content)
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
