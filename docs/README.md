# dxcore Documentation

Source of truth for dxcore docs.

## Layout

`guides/` holds the hand-written markdown guides. The source embeds them
into the rustdoc pages through `include_str!` doc attributes (see
`crates/core/src/lib.rs`). Doctest blocks inside the guides are compiled
and run by `cargo test`, so code examples stay correct. `README.md` is
this index.

The API docs are generated with rustdoc and are not stored in the repo.
Preview them locally with:

```bash
cargo doc --no-deps --workspace
# then open target/doc/dxcore/index.html i.e. the docs index on your browser.
```

The generated pages are published to GitHub Pages using workflow
`.github/workflows/publish-docs.yml`,
which runs on pushes to `main` and on release tags.
It stamps the version from the latest git tag,
so the published pages show the release version rather than the dev default.
The pages live at `divergex.github.io/dxcore` and the site embeds them at `/docs/dxcore`.

The docs cover only workspace crates (`--no-deps`), so third-party
dependencies are not documented (duh).
