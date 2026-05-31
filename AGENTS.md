# AGENTS.md

Guidance for coding agents working in this repository.

## Project

`filmlook` is a Rust CLI/library for applying deterministic film-emulation looks to images. The current product direction is CLI-first, with data-driven JSON recipes that can be bundled, edited, shared, and selected by id.

This project is an MVP and has not been released. Maintaining backwards compatibility is not required unless explicitly requested; compatibility shims, legacy aliases, and migration layers usually add noise at this stage.

The user prefers Rust for implementation. Do not introduce Python into the app pipeline unless explicitly asked.

## Layout

- `src/film.rs`: reusable image-processing engine, recipe schema, validation, built-in recipe registry, and unit tests.
- `src/main.rs`: CLI parsing, recipe loading, EXIF orientation, batch walking, metadata warnings, and image output.
- `src/lib.rs`: public library exports.
- `recipes/builtin/*.json`: bundled recipe definitions embedded into the binary with `include_str!`.
- `tests/cli.rs`: integration tests for CLI behavior.
- `test-images/`: local sample inputs and generated comparison outputs. Treat these as user-owned artifacts.

## Commands

Use these before handing off code changes:

```sh
cargo fmt
cargo test
cargo clippy --all-targets -- -D warnings
```

For recipe-only changes, also validate the changed recipe:

```sh
cargo run -- --validate-recipe recipes/builtin/<recipe-id>.json
```

For real-image exports and batch runs, prefer the optimized binary:

```sh
cargo build --release
target/release/filmlook input.jpg output.jpg --recipe <recipe-id>
```

## Git

Git commits should follow Conventional Commits, for example `feat: add recipe validation` or `fix: preserve EXIF orientation`.

## Adding A Built-In Recipe

When adding `recipes/builtin/<id>.json`:

1. Keep it valid against `FilmRecipe::validate` in `src/film.rs`.
2. Add the id to `BUILTIN_RECIPE_IDS`.
3. Add aliases in `canonical_builtin_recipe_id`.
4. Add an `include_str!` arm in `builtin_recipe_json`.
5. Update `README.md` recipe lists and aliases.
6. Add or update tests so `--list-recipes`, aliases, and distinct output behavior are covered.
7. Run `cargo fmt`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, and recipe validation.

Recipe ids should be lowercase kebab-case. Prefer descriptive aliases, but keep canonical ids stable once shipped.

## Recipe Tuning Notes

- Recipes are JSON data, not executable code.
- CLI flags override recipe defaults without modifying the JSON.
- Current controls cover global tone, channel tone, saturation, highlight desaturation, channel bias, shadow/highlight tint, hue-sector HSL shaping, density-aware grain, and halation threshold/color.
- The engine does not yet have perceptual color spaces, RAW input, or real ICC profile conversion.
- The CLI detects embedded ICC profiles and warns, but processing currently assumes decoded RGB is sRGB.
- Named film-stock recipes are inspired tunings for local/R&D use. Rename or review trademark/licensing before public commercial release.

## Generated Outputs

Do not delete or overwrite user test images unless explicitly asked. If generating comparisons, use clear suffixes such as:

```text
test-images/1-kodak-gold-200.jpg
test-images/1-ilford-hp5-plus-400.jpg
```

Avoid committing large generated output sets unless the user asks for fixtures or golden images.
