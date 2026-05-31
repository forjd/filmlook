use std::{
    fs::{self, File, create_dir_all},
    io::{BufReader, BufWriter},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use clap::Parser;
use exif::{In, Tag};
use filmlook::{FilmOptions, FilmRecipe, builtin_recipe, builtin_recipe_ids, process_image};
use image::{DynamicImage, ImageReader, RgbImage, codecs::jpeg::JpegEncoder};
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

#[derive(Debug)]
struct LoadedImage {
    image: DynamicImage,
    metadata: InputMetadata,
}

#[derive(Debug, Default)]
struct InputMetadata {
    orientation: Option<u32>,
    color_profile: ColorProfile,
}

#[derive(Debug, Default)]
enum ColorProfile {
    #[default]
    AssumedSrgb,
    Srgb,
    EmbeddedIcc(String),
    PngIcc(String),
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
    let path = Path::new(selector);
    if path.exists() {
        return load_recipe_from_path(path);
    }

    if let Some(recipe) = builtin_recipe(selector) {
        return Ok(recipe);
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

    if output.extension().is_some() {
        bail!("when input is a folder, output must be a folder path");
    }

    create_dir_all(output).with_context(|| format!("failed to create {}", output.display()))?;

    let mut walker = WalkDir::new(input).min_depth(1);
    if !cli.recursive {
        walker = walker.max_depth(1);
    }

    let mut processed = 0usize;
    let mut skipped = 0usize;
    for entry in walker {
        let entry = entry.with_context(|| format!("failed to walk {}", input.display()))?;
        let input_path = entry.path();

        if !entry.file_type().is_file() {
            continue;
        }

        if input_path.starts_with(output) || !is_supported_input(input_path) {
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
        eprintln!("{}: {}", input_path.display(), loaded.metadata.summary());
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

fn load_image(path: &Path, auto_orient: bool) -> Result<LoadedImage> {
    let metadata = read_input_metadata(path);
    let image = ImageReader::open(path)
        .with_context(|| format!("failed to open {}", path.display()))?
        .decode()
        .with_context(|| format!("failed to decode {}", path.display()))?;
    let image = if auto_orient {
        apply_orientation(image, metadata.orientation)
    } else {
        image
    };

    Ok(LoadedImage { image, metadata })
}

fn read_input_metadata(path: &Path) -> InputMetadata {
    InputMetadata {
        orientation: read_exif_orientation(path),
        color_profile: detect_color_profile(path),
    }
}

fn read_exif_orientation(path: &Path) -> Option<u32> {
    let file = File::open(path).ok()?;
    let mut reader = BufReader::new(file);
    let exif = exif::Reader::new().read_from_container(&mut reader).ok()?;
    let field = exif.get_field(Tag::Orientation, In::PRIMARY)?;

    field.value.get_uint(0)
}

fn apply_orientation(image: DynamicImage, orientation: Option<u32>) -> DynamicImage {
    match orientation.unwrap_or(1) {
        2 => image.fliph(),
        3 => image.rotate180(),
        4 => image.flipv(),
        5 => image.rotate90().fliph(),
        6 => image.rotate90(),
        7 => image.rotate90().flipv(),
        8 => image.rotate270(),
        _ => image,
    }
}

fn detect_color_profile(path: &Path) -> ColorProfile {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(_) => return ColorProfile::AssumedSrgb,
    };

    if bytes.starts_with(&[0xff, 0xd8])
        && let Some(profile) = read_jpeg_icc(&bytes)
    {
        return classify_icc_profile(&profile);
    }

    const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
    if bytes.starts_with(PNG_SIGNATURE) {
        return detect_png_color_profile(&bytes);
    }

    ColorProfile::AssumedSrgb
}

fn read_jpeg_icc(bytes: &[u8]) -> Option<Vec<u8>> {
    let mut offset = 2usize;
    let mut profile = Vec::new();

    while offset + 4 <= bytes.len() {
        if bytes[offset] != 0xff {
            offset += 1;
            continue;
        }

        while offset < bytes.len() && bytes[offset] == 0xff {
            offset += 1;
        }
        if offset >= bytes.len() {
            break;
        }

        let marker = bytes[offset];
        offset += 1;
        if marker == 0xda || marker == 0xd9 {
            break;
        }
        if matches!(marker, 0x01 | 0xd0..=0xd7) {
            continue;
        }
        if offset + 2 > bytes.len() {
            break;
        }

        let segment_len = u16::from_be_bytes([bytes[offset], bytes[offset + 1]]) as usize;
        if segment_len < 2 || offset + segment_len > bytes.len() {
            break;
        }

        let segment = &bytes[offset + 2..offset + segment_len];
        if marker == 0xe2 && segment.starts_with(b"ICC_PROFILE\0") && segment.len() > 14 {
            profile.extend_from_slice(&segment[14..]);
        }

        offset += segment_len;
    }

    if profile.is_empty() {
        None
    } else {
        Some(profile)
    }
}

fn detect_png_color_profile(bytes: &[u8]) -> ColorProfile {
    let mut offset = 8usize;
    while offset + 12 <= bytes.len() {
        let length = u32::from_be_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ]) as usize;
        let chunk_type = &bytes[offset + 4..offset + 8];
        let data_start = offset + 8;
        let data_end = data_start.saturating_add(length);
        if data_end + 4 > bytes.len() {
            break;
        }

        if chunk_type == b"sRGB" {
            return ColorProfile::Srgb;
        }
        if chunk_type == b"iCCP" {
            let name_end = bytes[data_start..data_end]
                .iter()
                .position(|byte| *byte == 0)
                .map(|position| data_start + position)
                .unwrap_or(data_start);
            let name = String::from_utf8_lossy(&bytes[data_start..name_end]).to_string();
            return ColorProfile::PngIcc(if name.is_empty() {
                "embedded PNG ICC profile".to_string()
            } else {
                name
            });
        }

        offset = data_end + 4;
    }

    ColorProfile::AssumedSrgb
}

fn classify_icc_profile(profile: &[u8]) -> ColorProfile {
    let text = String::from_utf8_lossy(profile);
    if text.contains("sRGB") || text.contains("IEC 61966") {
        ColorProfile::Srgb
    } else if text.contains("Display P3") {
        ColorProfile::EmbeddedIcc("Display P3".to_string())
    } else if text.contains("Adobe RGB") {
        ColorProfile::EmbeddedIcc("Adobe RGB".to_string())
    } else {
        ColorProfile::EmbeddedIcc("embedded ICC profile".to_string())
    }
}

fn save_image(image: &RgbImage, path: &Path, jpeg_quality: u8) -> Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;
    }

    if is_jpeg(path) {
        let file =
            File::create(path).with_context(|| format!("failed to create {}", path.display()))?;
        let writer = BufWriter::new(file);
        let mut encoder = JpegEncoder::new_with_quality(writer, jpeg_quality);
        encoder
            .encode_image(image)
            .with_context(|| format!("failed to encode {}", path.display()))?;
    } else {
        image
            .save(path)
            .with_context(|| format!("failed to save {}", path.display()))?;
    }

    Ok(())
}

impl InputMetadata {
    fn summary(&self) -> String {
        let orientation = self
            .orientation
            .map(|orientation| format!("EXIF orientation {orientation} applied"))
            .unwrap_or_else(|| "no EXIF orientation".to_string());

        format!("{orientation}; {}", self.color_profile.summary())
    }
}

impl ColorProfile {
    fn summary(&self) -> String {
        match self {
            ColorProfile::AssumedSrgb => "no embedded color profile; assuming sRGB".to_string(),
            ColorProfile::Srgb => "sRGB color profile".to_string(),
            ColorProfile::EmbeddedIcc(name) => format!("{name} ICC profile"),
            ColorProfile::PngIcc(name) => format!("{name} PNG ICC profile"),
        }
    }

    fn requires_assumption_warning(&self) -> bool {
        matches!(self, ColorProfile::EmbeddedIcc(_) | ColorProfile::PngIcc(_))
    }
}

fn is_supported_input(path: &Path) -> bool {
    extension_matches(
        path,
        &["jpg", "jpeg", "png", "tif", "tiff", "bmp", "webp", "qoi"],
    )
}

fn is_jpeg(path: &Path) -> bool {
    extension_matches(path, &["jpg", "jpeg"])
}

fn extension_matches(path: &Path, extensions: &[&str]) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            extensions
                .iter()
                .any(|candidate| extension.eq_ignore_ascii_case(candidate))
        })
        .unwrap_or(false)
}
