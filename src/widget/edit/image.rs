//! Images: checking, embedding or delegating, placeholders.

use super::super::{Action, EditKind, EditorState, Event, Mode};
use crate::doc::{Link, OBJ, Selection, Style};
use crate::i18n;
use crate::images::{self, ImageId, ImageMode, ImageRejection, ImageSource, PastedImage};
use crate::markdown;
use iced::Task;
use std::path::Path;
use std::sync::Arc;

/// File name without extension, as image description.
fn stem(name: Option<&str>) -> String {
    name.map(Path::new)
        .and_then(|path| path.file_stem())
        .and_then(|stem| stem.to_str())
        .unwrap_or("")
        .to_string()
}

impl EditorState {
    fn reject_image(&mut self, source: ImageSource, reason: ImageRejection) -> Task<Action> {
        self.pending_events
            .push(Event::ImageRejected { source, reason });
        Task::none()
    }

    pub(super) fn read_image_file(&mut self, path: &Path, source: ImageSource) -> Task<Action> {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_string);
        match std::fs::read(path) {
            Ok(bytes) => self.receive_image(bytes, source, name),
            Err(error) => self.reject_image(source, ImageRejection::Unreadable(error.to_string())),
        }
    }

    /// Checks an image and embeds it or hands it to the application.
    pub(super) fn receive_image(
        &mut self,
        bytes: Vec<u8>,
        source: ImageSource,
        name: Option<String>,
    ) -> Task<Action> {
        let settings = self.image_settings.clone();
        if !settings.enabled() {
            return self.reject_image(source, ImageRejection::Disabled);
        }
        let Some(mime) = images::sniff_mime(&bytes) else {
            return self.reject_image(source, ImageRejection::UnsupportedType("unknown".into()));
        };
        if !settings.accepts(mime) {
            return self.reject_image(source, ImageRejection::UnsupportedType(mime.into()));
        }
        if bytes.len() > settings.max_bytes {
            let reason = ImageRejection::TooLarge {
                size: bytes.len(),
                max: settings.max_bytes,
            };
            return self.reject_image(source, reason);
        }

        let alt = stem(name.as_deref());
        match settings.mode {
            ImageMode::Disabled => Task::none(),
            ImageMode::Embed => {
                let url = images::data_url(mime, &bytes);
                self.image_cache.borrow_mut().insert_bytes(&url, &bytes);
                self.insert_image(&url, &alt)
            }
            ImageMode::Delegate => {
                let id = self.new_image_id();
                let bytes: Arc<[u8]> = Arc::from(bytes);
                self.pending_images.insert(id, bytes.clone());
                let alt = i18n::text(self.language, "image-saving");
                let task = self.insert_image(&images::pending_url(id), &alt);
                self.pending_events.push(Event::ImagePasted(PastedImage {
                    id,
                    bytes,
                    mime,
                    source,
                    name,
                }));
                task
            }
        }
    }

    /// Inserts an image at the caret: formatted in WYSIWYG mode, as
    /// `![alt](url)` text in source mode and in code / HTML blocks.
    pub(crate) fn insert_image(&mut self, url: &str, alt: &str) -> Task<Action> {
        self.focused = true;
        self.begin_edit(EditKind::Other);
        let (a, b) = self.sel.range();
        let pos = self.doc.delete_range(a, b);
        let end = if self.doc.blocks[pos.block].leaf.is_plain() {
            let mut text = markdown::image_markdown(alt, url);
            if self.mode == Mode::Source {
                text = self.embedded.shorten(&text);
            }
            self.doc.insert_text(pos, &text, &Style::default())
        } else {
            let style = Style {
                image: Some(Link::new(url, None)),
                ..Style::default()
            };
            let chars: Vec<char> = if alt.is_empty() {
                vec![OBJ]
            } else {
                alt.chars().filter(|c| !c.is_control()).collect()
            };
            let mut at = pos;
            for c in chars {
                at = self.doc.insert_char(at, c, &style);
            }
            at
        };
        self.sel = Selection::caret(end);
        self.pending = None;
        self.last_edit = EditKind::None;
        self.after_edit()
    }

    /// Replaces the placeholder of `id` with `![alt](url)` (or removes it),
    /// also in the undo history.
    pub(super) fn resolve_image(&mut self, id: ImageId, replacement: Option<(&str, &str)>) {
        let bytes = self.pending_images.remove(&id);
        if let (Some((url, _)), Some(bytes)) = (replacement, &bytes) {
            self.image_cache.borrow_mut().insert_bytes(url, bytes);
        }

        let edits = images::replace_pending(&mut self.doc, Some(id), replacement);
        for snapshot in self.undo.iter_mut().chain(self.redo.iter_mut()) {
            images::replace_pending(&mut snapshot.doc, Some(id), replacement);
            snapshot.sel = Selection {
                anchor: snapshot.doc.clamp(snapshot.sel.anchor),
                head: snapshot.doc.clamp(snapshot.sel.head),
            };
        }
        if edits.is_empty() {
            return;
        }

        for (block, start, old, new) in edits {
            for pos in [&mut self.sel.anchor, &mut self.sel.head] {
                if pos.block == block && pos.offset >= start + old {
                    pos.offset = pos.offset - old + new;
                } else if pos.block == block && pos.offset > start {
                    pos.offset = start + new;
                }
            }
        }
        self.sel = Selection {
            anchor: self.doc.clamp(self.sel.anchor),
            head: self.doc.clamp(self.sel.head),
        };
        self.dirty = true;
        self.revision += 1;
        self.relayout();
    }
}
