# Web

The crate builds for `wasm32-unknown-unknown` without extra features;
dependencies are chosen per target (`tokio`, `arboard`, `png` only on the
desktop). `web-time` replaces `std::time::Instant`.

Your web app:

```toml
[dependencies]
iced = "0.14"
iced-markdown-editor = { version = "0.3", features = ["images"] }

[target.'cfg(target_arch = "wasm32")'.dependencies]
iced = { version = "0.14", features = ["webgl"] }
console_error_panic_hook = "0.1"
```

```bash
rustup target add wasm32-unknown-unknown
cargo install trunk
cd examples/web && trunk serve --open
```

Not available in the browser: clipboard images, file drops, local files
(`image_base`, `InsertImageFile`). Use `embed()` or upload the bytes.
