# Images

| `ImageSettings` | Pasted / dropped / inserted image |
|---|---|
| `disabled()` (default) | Rejected (`Event::ImageRejected`) |
| `embed()` | Written into the Markdown as a `data:` URL (the file contains the image as base64) |
| `delegate()` | Placeholder + `Event::ImagePasted`; the app stores the image and answers with `Command::ResolveImage { id, url, alt }` or `Command::CancelImage { id }` |

- The toolbar's **Image** button emits `Event::ImageRequested`; show a file
  dialog and send `Command::InsertImageFile(path)` (desktop) or
  `Command::InsertImage { bytes, name }` (any platform).
- `markdown()` never contains placeholders; `pending_images()` counts open ones.
- Display: `image_loader(|url| bytes)`, `data:` URLs, and files relative to
  `image_base(folder)` (desktop).
- In source mode long `data:` URLs are shown as `data:image/png;embedded,1`;
  the saved Markdown contains the full URL.

Examples: `images_folder`, `images_embed`, `images_upload`, `images_insert`, `examples/web`.
