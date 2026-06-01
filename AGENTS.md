# AGENTS.md

Guidance for coding agents working in this repository.

## Project

`filmlook` is a Rust film-emulation engine with three surfaces:

- a native Tauri desktop app in `apps/desktop`
- a deterministic CLI in `src/main.rs`
- a reusable library exported from `src/lib.rs`

Recipes are data-driven JSON files for tone, color, grain, halation, vignette, and monochrome conversion. The project is still an unreleased MVP, so do not add compatibility shims, aliases, or migration layers unless explicitly asked.

## Layout

- `src/film.rs`: processing engine, recipe schema and validation, built-in recipe registry, unit tests.
- `src/image_io.rs`: image loading, EXIF orientation, metadata/profile handling, image saving.
- `src/main.rs`: CLI parsing, recipe loading, batch walking, warnings, and output.
- `apps/desktop/src/`: React/Vite UI and Tauri API bridge.
- `apps/desktop/src-tauri/`: Tauri v2 shell and Rust commands that call the library.
- `recipes/builtin/*.json`: bundled recipe definitions embedded with `include_str!`.
- `tests/cli.rs`: integration tests for CLI behavior.
- `demos/` and `test-images/`: sample inputs and generated outputs. Treat them as user-owned artifacts.

## Commands

Before handing off Rust changes:

```sh
cargo fmt
cargo test
cargo clippy --all-targets -- -D warnings
```

For Tauri Rust changes:

```sh
cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml
cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml
```

For desktop UI changes:

```sh
cd apps/desktop
npm install
npm run build
```

Run the native app during development with:

```sh
cd apps/desktop
npm run tauri dev
```

For recipe-only changes, validate the changed file:

```sh
cargo run -- --validate-recipe recipes/builtin/<recipe-id>.json
```

For real-image exports and batch runs, prefer the optimized binary:

```sh
cargo build --release
target/release/filmlook input.jpg output.jpg --recipe <recipe-id>
```

## Built-In Recipes

When adding `recipes/builtin/<id>.json`:

1. Keep it valid against `FilmRecipe::validate`.
2. Add the id to `BUILTIN_RECIPE_IDS`.
3. Add an `include_str!` arm in `builtin_recipe_json`.
4. Update the README recipe list.
5. Add or update tests for `--list-recipes` and distinct output behavior.
6. Run formatting, tests, clippy, and recipe validation.

Recipe ids are lowercase kebab-case and selected by exact id.

## Recipe Notes

- Recipes are JSON data, not executable code.
- CLI flags and desktop controls override recipe defaults without modifying JSON.
- The engine currently assumes decoded RGB is sRGB. It detects embedded ICC profiles and warns, but does not perform full ICC conversion.
- RAW input and perceptual color spaces are not supported yet.
- Named film-stock recipes are inspired tunings for local/R&D use. Review trademark and licensing implications before public commercial release.

## Generated Outputs

Do not delete or overwrite user images unless explicitly asked. When generating comparisons, use clear suffixes such as:

```text
test-images/1-kodak-gold-200.jpg
test-images/1-ilford-hp5-plus-400.jpg
```

Avoid committing large generated output sets unless the user asks for fixtures or golden images.

## Git

Use Conventional Commits, for example `feat: add desktop preview controls` or `fix: preserve EXIF orientation`.
