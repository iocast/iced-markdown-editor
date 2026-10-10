//! Zoom settings and key bindings.
//!
//! ```
//! use iced_markdown_editor::{KeyBinding, ZoomSettings, ZoomShortcuts};
//!
//! let standard = ZoomSettings::default();
//! let off = ZoomSettings::disabled();
//! let custom = ZoomSettings {
//!     min: 0.5,
//!     max: 3.0,
//!     mouse_wheel: false,
//!     shortcuts: ZoomShortcuts {
//!         zoom_in: vec![KeyBinding::command("+"), KeyBinding::command_shift("i")],
//!         zoom_out: vec![KeyBinding::command("-"), KeyBinding::command_shift("o")],
//!         reset: vec![KeyBinding::command("0")],
//!     },
//!     ..ZoomSettings::default()
//! };
//! ```

use crate::widget::Command;
use iced::keyboard::{Key, Modifiers};

/// A key combination.
///
/// Character keys are compared case-insensitively, against both the key and
/// the key with modifiers applied (so `Ctrl` + `+` matches on keyboards
/// where `+` needs Shift). Ctrl, Alt and Logo must match exactly; Shift is
/// required when the binding contains it, otherwise ignored.
#[derive(Debug, Clone, PartialEq)]
pub struct KeyBinding {
    pub key: Key,
    pub modifiers: Modifiers,
}

impl KeyBinding {
    pub fn new(key: Key, modifiers: Modifiers) -> Self {
        Self { key, modifiers }
    }

    /// `Ctrl` (Windows, Linux) or `Cmd` (macOS) plus a character.
    pub fn command(character: &str) -> Self {
        Self::new(Key::Character(character.into()), Modifiers::COMMAND)
    }

    /// `Ctrl`/`Cmd` + `Shift` plus a character.
    pub fn command_shift(character: &str) -> Self {
        Self::new(
            Key::Character(character.into()),
            Modifiers::COMMAND | Modifiers::SHIFT,
        )
    }

    /// Whether the pressed key matches this binding.
    pub fn matches(&self, key: &Key, modified_key: &Key, modifiers: Modifiers) -> bool {
        let same_key = |pressed: &Key| match (self.key.as_ref(), pressed.as_ref()) {
            (Key::Character(a), Key::Character(b)) => a.eq_ignore_ascii_case(b),
            (a, b) => a == b,
        };

        (same_key(key) || same_key(modified_key))
            && modifiers.control() == self.modifiers.control()
            && modifiers.alt() == self.modifiers.alt()
            && modifiers.logo() == self.modifiers.logo()
            && (!self.modifiers.shift() || modifiers.shift())
    }
}

/// Keyboard shortcuts for zooming. Empty lists disable a shortcut.
#[derive(Debug, Clone, PartialEq)]
pub struct ZoomShortcuts {
    pub zoom_in: Vec<KeyBinding>,
    pub zoom_out: Vec<KeyBinding>,
    pub reset: Vec<KeyBinding>,
}

impl Default for ZoomShortcuts {
    /// `Ctrl`/`Cmd` + `+` or `=` (in), `-` (out), `0` (reset to 100 %).
    fn default() -> Self {
        Self {
            zoom_in: vec![KeyBinding::command("+"), KeyBinding::command("=")],
            zoom_out: vec![KeyBinding::command("-")],
            reset: vec![KeyBinding::command("0")],
        }
    }
}

impl ZoomShortcuts {
    /// No keyboard shortcuts.
    pub fn none() -> Self {
        Self {
            zoom_in: Vec::new(),
            zoom_out: Vec::new(),
            reset: Vec::new(),
        }
    }
}

/// Zoom behaviour.
#[derive(Debug, Clone, PartialEq)]
pub struct ZoomSettings {
    /// Master switch for shortcuts, mouse wheel, status bar control and the
    /// zoom commands. [`EditorState::set_zoom`](crate::EditorState::set_zoom)
    /// still works.
    pub enabled: bool,
    pub min: f32,
    pub max: f32,
    /// Steps used by zoom in / zoom out (ascending). When empty, each step
    /// multiplies or divides by 1.1.
    pub levels: Vec<f32>,
    /// `Ctrl`/`Cmd` + mouse wheel.
    pub mouse_wheel: bool,
    /// Zoom control (− 100 % +) in the status bar.
    pub controls: bool,
    pub shortcuts: ZoomShortcuts,
}

impl Default for ZoomSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            min: 0.25,
            max: 5.0,
            levels: vec![
                0.25, 0.33, 0.5, 0.67, 0.75, 0.8, 0.9, 1.0, 1.1, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0,
                4.0, 5.0,
            ],
            mouse_wheel: true,
            controls: true,
            shortcuts: ZoomShortcuts::default(),
        }
    }
}

impl ZoomSettings {
    /// Zoom fixed at its current value; no user interaction.
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Self::default()
        }
    }

    pub fn clamp(&self, zoom: f32) -> f32 {
        let (min, max) = (self.min.min(self.max), self.max.max(self.min));
        zoom.clamp(min.max(0.05), max.max(0.05))
    }

    /// The next zoom level in `direction` (positive = in).
    pub fn step(&self, zoom: f32, direction: i32) -> f32 {
        let next = if self.levels.is_empty() {
            if direction > 0 {
                zoom * 1.1
            } else {
                zoom / 1.1
            }
        } else if direction > 0 {
            self.levels
                .iter()
                .copied()
                .find(|level| *level > zoom * 1.001)
                .unwrap_or(self.max)
        } else {
            self.levels
                .iter()
                .rev()
                .copied()
                .find(|level| *level < zoom * 0.999)
                .unwrap_or(self.min)
        };
        self.clamp(next)
    }

    /// The zoom command bound to a key press, if any.
    pub(crate) fn command_for(
        &self,
        key: &Key,
        modified_key: &Key,
        modifiers: Modifiers,
    ) -> Option<Command> {
        if !self.enabled {
            return None;
        }
        let hit = |bindings: &[KeyBinding]| {
            bindings
                .iter()
                .any(|binding| binding.matches(key, modified_key, modifiers))
        };
        if hit(&self.shortcuts.zoom_in) {
            Some(Command::ZoomIn)
        } else if hit(&self.shortcuts.zoom_out) {
            Some(Command::ZoomOut)
        } else if hit(&self.shortcuts.reset) {
            Some(Command::ResetZoom)
        } else {
            None
        }
    }
}
