use std::{io::Cursor, path::PathBuf};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use filmlook::{
    FilmOptions, builtin_recipe, builtin_recipe_ids, load_image, process_image, save_image,
};
use image::{DynamicImage, GenericImageView, RgbImage, codecs::jpeg::JpegEncoder};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct RecipeSummary {
    id: String,
    name: String,
    description: String,
    tags: Vec<String>,
    monochrome: bool,
    defaults: FilmOptions,
}

#[derive(Debug, Serialize)]
pub struct PreviewResult {
    original_data_url: String,
    rendered_data_url: String,
    width: u32,
    height: u32,
    preview_width: u32,
    preview_height: u32,
    metadata_summary: String,
    warning: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ExportRequest {
    input_path: PathBuf,
    output_path: PathBuf,
    recipe_id: String,
    options: FilmOptions,
    quality: u8,
    auto_orient: bool,
}

#[derive(Debug, Serialize)]
pub struct ExportResult {
    output_path: String,
    warning: Option<String>,
}

#[tauri::command]
pub fn list_recipes() -> Result<Vec<RecipeSummary>, String> {
    builtin_recipe_ids()
        .iter()
        .map(|id| {
            let recipe = builtin_recipe(id).ok_or_else(|| format!("unknown recipe '{id}'"))?;
            let defaults = recipe.default_options();
            Ok(RecipeSummary {
                id: recipe.id,
                name: recipe.name,
                description: recipe.description,
                tags: recipe.tags,
                monochrome: recipe.monochrome,
                defaults,
            })
        })
        .collect()
}

#[tauri::command]
pub fn render_preview(
    input_path: PathBuf,
    recipe_id: String,
    options: FilmOptions,
    max_edge: u32,
) -> Result<PreviewResult, String> {
    let recipe =
        builtin_recipe(&recipe_id).ok_or_else(|| format!("unknown recipe '{recipe_id}'"))?;
    let loaded = load_image(&input_path, true).map_err(to_message)?;
    let (width, height) = loaded.image.dimensions();
    let preview = resize_for_preview(loaded.image, max_edge);
    let (preview_width, preview_height) = preview.dimensions();
    let original_data_url = encode_jpeg_data_url(&preview.to_rgb8(), 90)?;
    let rendered = process_image(&preview, &recipe, options);
    let rendered_data_url = encode_jpeg_data_url(&rendered, 90)?;
    let warning = loaded
        .metadata
        .color_profile
        .requires_assumption_warning()
        .then(|| {
            format!(
                "{}; processing currently assumes decoded RGB is sRGB",
                loaded.metadata.color_profile.summary()
            )
        });

    Ok(PreviewResult {
        original_data_url,
        rendered_data_url,
        width,
        height,
        preview_width,
        preview_height,
        metadata_summary: loaded.metadata.summary(true),
        warning,
    })
}

#[tauri::command]
pub fn export_image(request: ExportRequest) -> Result<ExportResult, String> {
    let recipe = builtin_recipe(&request.recipe_id)
        .ok_or_else(|| format!("unknown recipe '{}'", request.recipe_id))?;
    let loaded = load_image(&request.input_path, request.auto_orient).map_err(to_message)?;
    let rendered = process_image(&loaded.image, &recipe, request.options);
    let quality = request.quality.clamp(1, 100);
    save_image(&rendered, &request.output_path, quality).map_err(to_message)?;
    let warning = loaded
        .metadata
        .color_profile
        .requires_assumption_warning()
        .then(|| {
            format!(
                "{}; processing currently assumes decoded RGB is sRGB",
                loaded.metadata.color_profile.summary()
            )
        });

    Ok(ExportResult {
        output_path: request.output_path.display().to_string(),
        warning,
    })
}

fn resize_for_preview(image: DynamicImage, max_edge: u32) -> DynamicImage {
    let max_edge = max_edge.clamp(320, 2400);
    let (width, height) = image.dimensions();

    if width <= max_edge && height <= max_edge {
        image
    } else {
        image.thumbnail(max_edge, max_edge)
    }
}

fn encode_jpeg_data_url(image: &RgbImage, quality: u8) -> Result<String, String> {
    let mut bytes = Vec::new();
    let mut encoder = JpegEncoder::new_with_quality(Cursor::new(&mut bytes), quality.clamp(1, 100));
    encoder.encode_image(image).map_err(to_message)?;

    Ok(format!("data:image/jpeg;base64,{}", STANDARD.encode(bytes)))
}

fn to_message(error: impl std::fmt::Display) -> String {
    error.to_string()
}
