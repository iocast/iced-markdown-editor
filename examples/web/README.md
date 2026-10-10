# Web example

The editor in the browser (WebAssembly, WebGL).

```bash
rustup target add wasm32-unknown-unknown
cargo install trunk          # or: cargo binstall trunk
cd examples/web
trunk serve --open           # http://127.0.0.1:8080
trunk build --release        # static files in dist/
```

`cargo run` in this folder runs the same app on the desktop.

In the browser there is no file system and no desktop clipboard: pasting
images, dropping files and `image_base` are desktop features. Images are
embedded as `data:` URLs; the **Image** button uses the browser's file picker.
