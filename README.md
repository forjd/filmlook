# filmlook

`filmlook` is a CLI-first Rust MVP for applying a deterministic film-emulation look to images.

## Usage

Process one image:

```sh
cargo run -- input.jpg output.jpg
```

Tune the look:

```sh
cargo run -- input.jpg output.jpg \
  --recipe portra-400-35mm \
  --exposure-stops 0.3 \
  --contrast 1.05 \
  --shadows 0.15 \
  --highlight-rolloff 1.2 \
  --grain 0.4 \
  --grain-size 0.55 \
  --halation 0.3 \
  --vignette 0.15 \
  --seed 42
```

Batch process a folder:

```sh
cargo run -- ./input-photos ./output-photos --recursive --recipe cinematic-daylight
```

Use a shared or edited JSON recipe:

```sh
cargo run -- input.jpg output.jpg --recipe ./recipes/my-film.json
```

List bundled recipes:

```sh
cargo run -- --list-recipes
```

Validate a recipe file:

```sh
cargo run -- --validate-recipe ./recipes/my-film.json
```

## Performance

For real photos, prefer the optimized binary:

```sh
cargo build --release
target/release/filmlook input.jpg output.jpg --recipe portra-400-35mm
```

`cargo run` uses Cargo's dev profile. This repo sets a light dev optimization level so local runs are usable, but `--release` is still the right path for final exports and batch jobs.

## Recipes

Built-in recipes live in `recipes/builtin/*.json` and are also embedded into the binary:

- `neutral`
- `clean-negative`
- `portra-400-35mm`
- `kodak-gold-200`
- `ilford-hp5-plus-400`
- `cinematic-daylight`
- `tungsten-night`
- `slide`
- `consumer-soft`
- `mono`

The old MVP `--preset` flag still works as an alias for `--recipe`, and old names still resolve:

- `clean` -> `neutral`
- `warm-print` -> `clean-negative`
- `cool-chrome` -> `cinematic-daylight`
- `portra400` -> `portra-400-35mm`
- `gold200` -> `kodak-gold-200`
- `hp5` -> `ilford-hp5-plus-400`

Named stock recipes such as `portra-400-35mm`, `kodak-gold-200`, and `ilford-hp5-plus-400` are inspired tunings for local/R&D use. Rename them before public commercial release unless trademark/licensing has been reviewed.

## Recipe Format

Recipes are JSON data, not executable code. A recipe contains:

- metadata: `schema_version`, `id`, `name`, `description`, `author`, `tags`, `aliases`
- defaults: initial values for strength, grain, halation, vignette, and similar controls
- tone: exposure offset, toe, shadow lift, midtone contrast, shoulder, white point
- channel tone: red/green/blue response differences
- color: saturation, highlight desaturation, channel bias, shadow tint, highlight tint, optional hue-sector shaping
- grain: density response and grain size bias
- halation: color and threshold
- `monochrome`

CLI controls override recipe defaults without changing the JSON file. For example:

```sh
filmlook input.jpg output.jpg --recipe portra-400-35mm --grain 0.15
```

Hue sectors can target specific color families in recipe JSON:

```json
{
  "hue_sectors": [
    {
      "center_degrees": 120.0,
      "width_degrees": 70.0,
      "softness": 0.5,
      "amount": 1.0,
      "hue_shift_degrees": -8.0,
      "saturation_scale": 0.92,
      "luminance_scale": 1.02
    }
  ]
}
```

## Controls

- `--strength`: overall look amount, `0.0..1.0`
- `--exposure-stops`: virtual exposure in stops, `-3.0..3.0`
- `--contrast`: midtone contrast multiplier
- `--shadows`: toe/shadow adjustment, `-1.0..1.0`
- `--highlight-rolloff`: shoulder strength multiplier, `0.0..2.0`
- `--grain`: density-aware grain amount, `0.0..1.0`
- `--grain-size`: grain scale, `0.0..1.0`
- `--halation`: highlight-edge halation amount, `0.0..1.0`
- `--vignette`: edge darkening amount, `0.0..1.0`
- `--seed`: deterministic grain seed
- `--quality`: JPEG output quality

## Current Engine

- `src/film.rs` contains the reusable image-processing pipeline.
- `src/main.rs` contains CLI parsing, metadata loading, batch walking, and image saving.
- Input pixels are converted from sRGB-like values to linear RGB before the look is applied.
- The tone response uses explicit toe, midtone slope, and shoulder parameters.
- Recipes use channel-specific tone response, tinting, saturation, hue-sector shaping, grain response, and halation thresholds.
- Grain is deterministic, density-aware, color-layer aware, and multi-scale.
- Halation is driven by bright high-contrast edges, not a global glow.
- EXIF orientation is applied by default; use `--no-auto-orient` to disable it.
- Embedded ICC profile detection is present as groundwork, but the engine currently processes decoded RGB as sRGB.

## Likely Next Steps

- Add proper ICC profile conversion instead of only detection/warnings.
- Add golden-image snapshot tests with a curated sample set.
- Add optional output format conversion for batch mode.
- Add user recipe discovery from an app config directory.
- Add RAW input through a separate pro pipeline.

## License

MIT © Forjd.
