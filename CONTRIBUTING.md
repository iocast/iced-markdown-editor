# Contributing

## Previewing the documentation

### API docs

```bash
cargo doc --all-features --no-deps --open
```

To match docs.rs exactly, including the "Available on crate feature … only" badges, build on
nightly with the `docsrs` cfg from `[package.metadata.docs.rs]`:

```bash
rustup toolchain install nightly   # once
cargo +nightly rustdoc --all-features --open -- --cfg docsrs
```

The output is written to `target/doc/iced_markdown_editor/index.html`.

### Guides

The guides in [`docs/`](docs/README.md) are plain Markdown and are not part of rustdoc; any
Markdown preview shows them as GitHub will.

### Website

See [`website/README.md`](website/README.md). `zola serve` serves the site at
<http://127.0.0.1:1111>.
