use std::{
    fs::{self, create_dir_all},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use clap::Parser;
use filmlook::{
    FilmOptions, FilmRecipe, builtin_recipe, builtin_recipe_ids, is_supported_input,
    is_within_canonical_path, load_image, process_image, save_image,
};
use walkdir::WalkDir;

#[derive(Debug, Parser)]
#[command(
    name = "filmlook",
    version,
    about = "Apply a film-emulation look to images."
)]
struct Cli {
    /// Input image path, or a folder for batch processing.
    input: Option<PathBuf>,

    /// Output image path, or an output folder when the input is a folder.
    output: Option<PathBuf>,

    /// Built-in recipe id or path to a JSON recipe file.
    #[arg(long, value_name = "ID_OR_PATH")]
    recipe: Option<String>,

    /// Overall amount of the look, from 0.0 to 1.0.
    #[arg(long)]
    strength: Option<f32>,

    /// Virtual exposure adjustment in stops.
    #[arg(long)]
    exposure_stops: Option<f32>,

    /// Midtone contrast multiplier.
    #[arg(long)]
    contrast: Option<f32>,

    /// Shadow/toe adjustment from -1.0 for denser shadows to 1.0 for lifted shadows.
    #[arg(long)]
    shadows: Option<f32>,

    /// Highlight shoulder multiplier from 0.0 to 2.0.
    #[arg(long)]
    highlight_rolloff: Option<f32>,

    /// Grain amount, from 0.0 to 1.0.
    #[arg(long)]
    grain: Option<f32>,

    /// Grain size, from 0.0 for fine to 1.0 for coarse.
    #[arg(long)]
    grain_size: Option<f32>,

    /// Warm highlight-edge halation amount, from 0.0 to 1.0.
    #[arg(long)]
    halation: Option<f32>,

    /// Edge darkening amount, from 0.0 to 1.0.
    #[arg(long)]
    vignette: Option<f32>,

    /// Deterministic grain seed.
    #[arg(long, default_value_t = 1)]
    seed: u32,

    /// JPEG output quality, used only for .jpg and .jpeg outputs.
    #[arg(long, default_value_t = 92)]
    quality: u8,

    /// Recurse into subdirectories when the input is a folder.
    #[arg(long)]
    recursive: bool,

    /// Do not apply EXIF orientation before processing.
    #[arg(long)]
    no_auto_orient: bool,

    /// Print input metadata and per-file batch output.
    #[arg(long)]
    verbose: bool,

    /// List bundled recipe ids and names.
    #[arg(long)]
    list_recipes: bool,

    /// Validate a JSON recipe file and exit.
    #[arg(long, value_name = "PATH")]
    validate_recipe: Option<PathBuf>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.list_recipes {
        list_recipes();
        return Ok(());
    }

    if let Some(path) = &cli.validate_recipe {
        let recipe = load_recipe_from_path(path)?;
        println!("{} is valid ({})", path.display(), recipe.id);
        return Ok(());
    }

    let input = cli
        .input
        .as_ref()
        .context("missing input path; use --list-recipes to inspect recipes")?;
    let output = cli.output.as_ref().context("missing output path")?;
    let recipe = resolve_recipe(cli.recipe.as_deref().unwrap_or("portra-400-35mm"))?;
    let options = options_from_cli(recipe.default_options(), &cli);

    if input.is_dir() {
        process_batch(&cli, &recipe, options)?;
    } else {
        let output = if output.is_dir() {
            output.join(
                input
                    .file_name()
                    .context("input path does not have a file name")?,
            )
        } else {
            output.clone()
        };
        process_one(input, &output, &recipe, options, &cli)?;
    }

    Ok(())
}

fn list_recipes() {
    for id in builtin_recipe_ids() {
        if let Some(recipe) = builtin_recipe(id) {
            println!("{:<22} {}", recipe.id, recipe.name);
        }
    }
}

fn resolve_recipe(selector: &str) -> Result<FilmRecipe> {
    if let Some(recipe) = builtin_recipe(selector) {
        return Ok(recipe);
    }

    let path = Path::new(selector);
    if path.exists() {
        return load_recipe_from_path(path);
    }

    bail!(
        "unknown recipe '{selector}'. Run `filmlook --list-recipes` for exact ids or pass a path to a JSON recipe file"
    )
}

fn load_recipe_from_path(path: &Path) -> Result<FilmRecipe> {
    let json =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    FilmRecipe::from_json_str(&json).with_context(|| format!("invalid recipe {}", path.display()))
}

fn options_from_cli(mut options: FilmOptions, cli: &Cli) -> FilmOptions {
    if let Some(value) = cli.strength {
        options.strength = value;
    }
    if let Some(value) = cli.exposure_stops {
        options.exposure_stops = value;
    }
    if let Some(value) = cli.contrast {
        options.contrast = value;
    }
    if let Some(value) = cli.shadows {
        options.shadows = value;
    }
    if let Some(value) = cli.highlight_rolloff {
        options.highlight_rolloff = value;
    }
    if let Some(value) = cli.grain {
        options.grain = value;
    }
    if let Some(value) = cli.grain_size {
        options.grain_size = value;
    }
    if let Some(value) = cli.halation {
        options.halation = value;
    }
    if let Some(value) = cli.vignette {
        options.vignette = value;
    }
    options.seed = cli.seed;
    options
}

fn process_batch(cli: &Cli, recipe: &FilmRecipe, options: FilmOptions) -> Result<()> {
    let input = cli.input.as_ref().context("missing input path")?;
    let output = cli.output.as_ref().context("missing output path")?;

    if output.exists() && !output.is_dir() {
        bail!("when input is a folder, output must be a folder path");
    }

    create_dir_all(output).with_context(|| format!("failed to create {}", output.display()))?;
    let input_canonical = input
        .canonicalize()
        .with_context(|| format!("failed to resolve {}", input.display()))?;
    let output_canonical = output
        .canonicalize()
        .with_context(|| format!("failed to resolve {}", output.display()))?;
    if input_canonical == output_canonical {
        bail!("batch output folder must be different from input folder");
    }

    let mut walker = WalkDir::new(input).min_depth(1);
    if !cli.recursive {
        walker = walker.max_depth(1);
    }

    let mut processed = 0usize;
    let mut skipped = 0usize;
    for entry in walker
        .into_iter()
        .filter_entry(|entry| !is_within_canonical_path(entry.path(), &output_canonical))
    {
        let entry = entry.with_context(|| format!("failed to walk {}", input.display()))?;
        let input_path = entry.path();

        if !entry.file_type().is_file() {
            continue;
        }

        if !is_supported_input(input_path) {
            skipped += 1;
            continue;
        }

        let relative_path = input_path
            .strip_prefix(input)
            .with_context(|| format!("failed to build output path for {}", input_path.display()))?;
        let output_path = output.join(relative_path);
        process_one(input_path, &output_path, recipe, options, cli)?;
        processed += 1;
    }

    if processed == 0 {
        bail!(
            "no supported images found in {}{}",
            input.display(),
            if cli.recursive { "" } else { " at depth 1" }
        );
    }

    eprintln!("Processed {processed} image(s), skipped {skipped} file(s)");
    Ok(())
}

fn process_one(
    input_path: &Path,
    output_path: &Path,
    recipe: &FilmRecipe,
    options: FilmOptions,
    cli: &Cli,
) -> Result<()> {
    let loaded = load_image(input_path, !cli.no_auto_orient)
        .with_context(|| format!("failed to load {}", input_path.display()))?;
    if cli.verbose {
        eprintln!(
            "{}: {}",
            input_path.display(),
            loaded.metadata.summary(!cli.no_auto_orient)
        );
    } else if loaded.metadata.color_profile.requires_assumption_warning() {
        eprintln!(
            "Warning: {} has {}; processing currently assumes decoded RGB is sRGB",
            input_path.display(),
            loaded.metadata.color_profile.summary()
        );
    }

    let rendered = process_image(&loaded.image, recipe, options);
    save_image(&rendered, output_path, cli.quality.clamp(1, 100))?;

    if cli.verbose || cli.input.as_ref().is_some_and(|input| !input.is_dir()) {
        eprintln!("Wrote {}", output_path.display());
    }

    Ok(())
}
