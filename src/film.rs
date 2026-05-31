use image::{DynamicImage, RgbImage};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::{error::Error, fmt};

const OUTPUT_LUT_STEPS: usize = 8192;
const OUTPUT_LUT_LEN: usize = OUTPUT_LUT_STEPS + 1;

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FilmOptions {
    pub strength: f32,
    pub exposure_stops: f32,
    pub contrast: f32,
    pub shadows: f32,
    pub highlight_rolloff: f32,
    pub grain: f32,
    pub grain_size: f32,
    pub halation: f32,
    pub vignette: f32,
    pub seed: u32,
}

impl Default for FilmOptions {
    fn default() -> Self {
        Self {
            strength: 0.8,
            exposure_stops: 0.0,
            contrast: 1.0,
            shadows: 0.0,
            highlight_rolloff: 1.0,
            grain: 0.35,
            grain_size: 0.45,
            halation: 0.25,
            vignette: 0.15,
            seed: 1,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FilmRecipe {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub defaults: FilmOptions,
    pub tone: ToneRecipe,
    pub channel_tone: ChannelToneSet,
    pub color: ColorRecipe,
    pub grain: GrainResponse,
    pub halation: HalationRecipe,
    #[serde(default)]
    pub monochrome: bool,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ToneRecipe {
    pub exposure_stops: f32,
    pub toe: f32,
    pub shadow_lift: f32,
    pub mid_contrast: f32,
    pub shoulder: f32,
    pub shoulder_start: f32,
    pub white_point: f32,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ChannelToneSet {
    pub red: ChannelTone,
    pub green: ChannelTone,
    pub blue: ChannelTone,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ChannelTone {
    pub exposure_stops: f32,
    pub toe: f32,
    pub contrast: f32,
    pub shoulder: f32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ColorRecipe {
    pub saturation: f32,
    #[serde(default)]
    pub highlight_desaturation: f32,
    pub channel_bias: [f32; 3],
    pub shadow_tint: [f32; 3],
    pub highlight_tint: [f32; 3],
    #[serde(default)]
    pub hue_sectors: Vec<HueSectorRecipe>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HueSectorRecipe {
    pub center_degrees: f32,
    pub width_degrees: f32,
    #[serde(default = "default_hue_sector_softness")]
    pub softness: f32,
    #[serde(default = "default_hue_sector_amount")]
    pub amount: f32,
    #[serde(default)]
    pub hue_shift_degrees: f32,
    #[serde(default = "default_hue_sector_scale")]
    pub saturation_scale: f32,
    #[serde(default = "default_hue_sector_scale")]
    pub luminance_scale: f32,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GrainResponse {
    pub shadows: f32,
    pub midtones: f32,
    pub highlights: f32,
    pub chroma: f32,
    pub size_bias: f32,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HalationRecipe {
    pub color: [f32; 3],
    pub threshold: f32,
}

#[derive(Debug)]
pub enum RecipeError {
    Json(serde_json::Error),
    Invalid(String),
}

impl fmt::Display for RecipeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RecipeError::Json(error) => write!(formatter, "{error}"),
            RecipeError::Invalid(message) => formatter.write_str(message),
        }
    }
}

impl Error for RecipeError {}

impl From<serde_json::Error> for RecipeError {
    fn from(error: serde_json::Error) -> Self {
        RecipeError::Json(error)
    }
}

#[derive(Debug, Clone, Copy)]
struct EffectiveTone {
    toe: f32,
    shadow_lift: f32,
    mid_contrast: f32,
    shoulder: f32,
    shoulder_start: f32,
    white_point: f32,
}

struct RenderTables {
    srgb_to_linear: [f32; 256],
    exposed_linear: [f32; 256],
    tone: [[f32; 256]; 3],
    linear_to_srgb: [u8; OUTPUT_LUT_LEN],
}

pub const BUILTIN_RECIPE_IDS: &[&str] = &[
    "portra-400-35mm",
    "kodak-gold-200",
    "kodak-ektar-100",
    "ilford-hp5-plus-400",
    "kodak-tri-x-400",
    "cinestill-800t",
];

pub fn builtin_recipe_ids() -> &'static [&'static str] {
    BUILTIN_RECIPE_IDS
}

pub fn builtin_recipe_json(id: &str) -> Option<&'static str> {
    match id {
        "portra-400-35mm" => Some(include_str!("../recipes/builtin/portra-400-35mm.json")),
        "kodak-gold-200" => Some(include_str!("../recipes/builtin/kodak-gold-200.json")),
        "kodak-ektar-100" => Some(include_str!("../recipes/builtin/kodak-ektar-100.json")),
        "ilford-hp5-plus-400" => Some(include_str!("../recipes/builtin/ilford-hp5-plus-400.json")),
        "kodak-tri-x-400" => Some(include_str!("../recipes/builtin/kodak-tri-x-400.json")),
        "cinestill-800t" => Some(include_str!("../recipes/builtin/cinestill-800t.json")),
        _ => None,
    }
}

pub fn builtin_recipe(id: &str) -> Option<FilmRecipe> {
    builtin_recipe_json(id)
        .map(|json| FilmRecipe::from_json_str(json).expect("built-in recipe JSON is valid"))
}

impl FilmRecipe {
    pub fn from_json_str(json: &str) -> Result<Self, RecipeError> {
        let recipe: Self = serde_json::from_str(json)?;
        recipe.validate()?;
        Ok(recipe)
    }

    pub fn validate(&self) -> Result<(), RecipeError> {
        if self.schema_version != 1 {
            return Err(RecipeError::Invalid(format!(
                "unsupported recipe schema_version {}; expected 1",
                self.schema_version
            )));
        }
        if self.id.trim().is_empty() {
            return Err(RecipeError::Invalid(
                "recipe id cannot be empty".to_string(),
            ));
        }
        if self.name.trim().is_empty() {
            return Err(RecipeError::Invalid(
                "recipe name cannot be empty".to_string(),
            ));
        }

        validate_options(&self.defaults, "defaults")?;
        validate_range("tone.exposure_stops", self.tone.exposure_stops, -3.0, 3.0)?;
        validate_range("tone.toe", self.tone.toe, 0.0, 2.0)?;
        validate_range("tone.shadow_lift", self.tone.shadow_lift, 0.0, 0.2)?;
        validate_range("tone.mid_contrast", self.tone.mid_contrast, 0.25, 3.0)?;
        validate_range("tone.shoulder", self.tone.shoulder, 0.0, 3.0)?;
        validate_range("tone.shoulder_start", self.tone.shoulder_start, 0.0, 1.2)?;
        validate_range("tone.white_point", self.tone.white_point, 0.1, 2.0)?;

        for (name, channel) in self.channel_tone.channels_with_names() {
            validate_range(
                &format!("channel_tone.{name}.exposure_stops"),
                channel.exposure_stops,
                -2.0,
                2.0,
            )?;
            validate_range(&format!("channel_tone.{name}.toe"), channel.toe, -1.0, 1.0)?;
            validate_range(
                &format!("channel_tone.{name}.contrast"),
                channel.contrast,
                -1.0,
                1.0,
            )?;
            validate_range(
                &format!("channel_tone.{name}.shoulder"),
                channel.shoulder,
                -1.0,
                1.0,
            )?;
        }

        validate_range("color.saturation", self.color.saturation, 0.0, 2.5)?;
        validate_range(
            "color.highlight_desaturation",
            self.color.highlight_desaturation,
            0.0,
            1.0,
        )?;
        validate_array("color.channel_bias", self.color.channel_bias, 0.0, 3.0)?;
        validate_array("color.shadow_tint", self.color.shadow_tint, -1.0, 1.0)?;
        validate_array("color.highlight_tint", self.color.highlight_tint, -1.0, 1.0)?;
        for (index, sector) in self.color.hue_sectors.iter().enumerate() {
            let prefix = format!("color.hue_sectors[{index}]");
            validate_range(
                &format!("{prefix}.center_degrees"),
                sector.center_degrees,
                0.0,
                360.0,
            )?;
            validate_range(
                &format!("{prefix}.width_degrees"),
                sector.width_degrees,
                0.0,
                360.0,
            )?;
            validate_range(&format!("{prefix}.softness"), sector.softness, 0.0, 1.0)?;
            validate_range(&format!("{prefix}.amount"), sector.amount, 0.0, 1.0)?;
            validate_range(
                &format!("{prefix}.hue_shift_degrees"),
                sector.hue_shift_degrees,
                -180.0,
                180.0,
            )?;
            validate_range(
                &format!("{prefix}.saturation_scale"),
                sector.saturation_scale,
                0.0,
                3.0,
            )?;
            validate_range(
                &format!("{prefix}.luminance_scale"),
                sector.luminance_scale,
                0.0,
                3.0,
            )?;
        }

        validate_range("grain.shadows", self.grain.shadows, 0.0, 3.0)?;
        validate_range("grain.midtones", self.grain.midtones, 0.0, 3.0)?;
        validate_range("grain.highlights", self.grain.highlights, 0.0, 3.0)?;
        validate_range("grain.chroma", self.grain.chroma, 0.0, 1.0)?;
        validate_range("grain.size_bias", self.grain.size_bias, 0.0, 1.0)?;

        validate_array("halation.color", self.halation.color, 0.0, 1.0)?;
        validate_range("halation.threshold", self.halation.threshold, 0.0, 1.2)?;

        Ok(())
    }

    pub fn default_options(&self) -> FilmOptions {
        self.defaults.clamped()
    }
}

impl ChannelToneSet {
    fn channels(self) -> [ChannelTone; 3] {
        [self.red, self.green, self.blue]
    }

    fn channels_with_names(self) -> [(&'static str, ChannelTone); 3] {
        [
            ("red", self.red),
            ("green", self.green),
            ("blue", self.blue),
        ]
    }
}

pub fn process_image(input: &DynamicImage, recipe: &FilmRecipe, options: FilmOptions) -> RgbImage {
    let options = options.clamped();
    let source = input.to_rgb8();
    let (width, height) = source.dimensions();
    let width_usize = width as usize;
    let tables = build_render_tables(recipe, options);
    let linear_luma = build_linear_luma(&source, &tables.srgb_to_linear);
    let halation_mask = build_halation_mask(
        &linear_luma,
        width as usize,
        height as usize,
        recipe.halation.threshold,
        options.halation,
    );

    let source_raw = source.as_raw();
    let mut output_raw = vec![0; source_raw.len()];
    output_raw
        .par_chunks_mut(3)
        .enumerate()
        .for_each(|(idx, pixel)| {
            let source_idx = idx * 3;
            let red = source_raw[source_idx] as usize;
            let green = source_raw[source_idx + 1] as usize;
            let blue = source_raw[source_idx + 2] as usize;
            let x = (idx % width_usize) as u32;
            let y = (idx / width_usize) as u32;

            let original = [
                tables.exposed_linear[red],
                tables.exposed_linear[green],
                tables.exposed_linear[blue],
            ];
            let mut color = [
                tables.tone[0][red],
                tables.tone[1][green],
                tables.tone[2][blue],
            ];

            color = mix_color(original, color, options.strength);
            color = adjust_saturation(color, mix(1.0, recipe.color.saturation, options.strength));
            color = desaturate_highlights(color, recipe.color.highlight_desaturation, options);

            let lum = luminance(color).clamp(0.0, 1.0);
            let shadow_weight = 1.0 - smoothstep(0.12, 0.58, lum);
            let highlight_weight = smoothstep(0.5, 0.96, lum);
            for (channel, value) in color.iter_mut().enumerate() {
                *value *= mix(1.0, recipe.color.channel_bias[channel], options.strength);
                *value += options.strength
                    * (recipe.color.shadow_tint[channel] * shadow_weight
                        + recipe.color.highlight_tint[channel] * highlight_weight);
            }
            color = apply_hue_sectors(color, &recipe.color.hue_sectors, options.strength);

            if recipe.monochrome {
                let gray = luminance(color);
                color = [gray, gray, gray];
            }

            let idx = y as usize * width as usize + x as usize;
            add_halation(&mut color, halation_mask[idx], recipe, options);

            let vignette = vignette_factor(x, y, width, height, options.vignette, options.strength);
            for channel in &mut color {
                *channel *= vignette;
            }

            if options.grain > 0.0 {
                add_grain(&mut color, x, y, options, recipe.grain);
            }

            pixel[0] = linear_to_u8(color[0], &tables.linear_to_srgb);
            pixel[1] = linear_to_u8(color[1], &tables.linear_to_srgb);
            pixel[2] = linear_to_u8(color[2], &tables.linear_to_srgb);
        });

    RgbImage::from_raw(width, height, output_raw).expect("output buffer length matches dimensions")
}

impl FilmOptions {
    fn clamped(self) -> Self {
        Self {
            strength: clamp_unit_or(self.strength, 0.8),
            exposure_stops: clamp_or(self.exposure_stops, 0.0, -3.0, 3.0),
            contrast: clamp_or(self.contrast, 1.0, 0.25, 2.5),
            shadows: clamp_or(self.shadows, 0.0, -1.0, 1.0),
            highlight_rolloff: clamp_or(self.highlight_rolloff, 1.0, 0.0, 2.0),
            grain: clamp_unit_or(self.grain, 0.35),
            grain_size: clamp_unit_or(self.grain_size, 0.45),
            halation: clamp_unit_or(self.halation, 0.25),
            vignette: clamp_unit_or(self.vignette, 0.15),
            seed: self.seed,
        }
    }
}

fn build_render_tables(recipe: &FilmRecipe, options: FilmOptions) -> RenderTables {
    let mut srgb_to_linear = [0.0; 256];
    for (input, value) in srgb_to_linear.iter_mut().enumerate() {
        *value = srgb_to_linear_value(input as f32 / 255.0);
    }

    let user_exposure = 2.0_f32.powf(options.exposure_stops);
    let mut exposed_linear = [0.0; 256];
    for (input, value) in exposed_linear.iter_mut().enumerate() {
        *value = srgb_to_linear[input] * user_exposure;
    }

    let mut tone = [[0.0; 256]; 3];
    let channel_tones = recipe.channel_tone.channels();
    for (channel, table) in tone.iter_mut().enumerate() {
        let channel_tone = channel_tones[channel];
        let exposure = 2.0_f32.powf(recipe.tone.exposure_stops + channel_tone.exposure_stops);
        let effective = effective_tone(recipe.tone, channel_tone, options);
        for (input, value) in table.iter_mut().enumerate() {
            *value = apply_film_tone(exposed_linear[input] * exposure, effective);
        }
    }

    let mut linear_to_srgb = [0; OUTPUT_LUT_LEN];
    for (input, value) in linear_to_srgb.iter_mut().enumerate() {
        let linear = input as f32 / OUTPUT_LUT_STEPS as f32;
        *value = to_u8(linear_to_srgb_value(linear));
    }

    RenderTables {
        srgb_to_linear,
        exposed_linear,
        tone,
        linear_to_srgb,
    }
}

fn effective_tone(base: ToneRecipe, channel: ChannelTone, options: FilmOptions) -> EffectiveTone {
    let shadow_lift =
        base.shadow_lift + options.shadows.max(0.0) * 0.06 - (-options.shadows).max(0.0) * 0.018;
    let toe = base.toe + channel.toe + (-options.shadows).max(0.0) * 0.32
        - options.shadows.max(0.0) * 0.14;

    EffectiveTone {
        toe: toe.clamp(0.0, 1.35),
        shadow_lift: shadow_lift.clamp(0.0, 0.09),
        mid_contrast: ((base.mid_contrast + channel.contrast) * options.contrast).clamp(0.35, 2.5),
        shoulder: ((base.shoulder + channel.shoulder).max(0.0) * options.highlight_rolloff)
            .clamp(0.0, 2.2),
        shoulder_start: base.shoulder_start.clamp(0.42, 0.94),
        white_point: base.white_point.clamp(0.72, 1.35),
    }
}

fn apply_film_tone(value: f32, tone: EffectiveTone) -> f32 {
    let mut x = value.max(0.0);

    let toe_weight = 1.0 - smoothstep(0.0, 0.34, x);
    let toe_power = 1.0 + tone.toe * 1.15;
    let toe_value = (x + tone.shadow_lift).powf(toe_power) + tone.shadow_lift * 0.35;
    x = mix(x, toe_value, toe_weight * tone.toe.clamp(0.0, 1.0));

    x = apply_contrast(x, tone.mid_contrast);

    if x > tone.shoulder_start {
        let excess = x - tone.shoulder_start;
        let compression = 0.72 + tone.shoulder * 2.45;
        x = tone.shoulder_start + excess / (1.0 + excess * compression);
    }

    (x / tone.white_point).max(0.0)
}

fn build_linear_luma(image: &RgbImage, srgb_to_linear: &[f32; 256]) -> Vec<f32> {
    image
        .as_raw()
        .par_chunks_exact(3)
        .map(|rgb| {
            luminance([
                srgb_to_linear[rgb[0] as usize],
                srgb_to_linear[rgb[1] as usize],
                srgb_to_linear[rgb[2] as usize],
            ])
        })
        .collect()
}

fn build_halation_mask(
    luma: &[f32],
    width: usize,
    height: usize,
    threshold: f32,
    amount: f32,
) -> Vec<f32> {
    if amount <= 0.0 || width == 0 || height == 0 {
        return vec![0.0; luma.len()];
    }

    let mut edge_mask = vec![0.0; luma.len()];
    for y in 0..height {
        for x in 0..width {
            let idx = y * width + x;
            let center = luma[idx];
            let left = luma[y * width + x.saturating_sub(1)];
            let right = luma[y * width + (x + 1).min(width - 1)];
            let up = luma[y.saturating_sub(1) * width + x];
            let down = luma[(y + 1).min(height - 1) * width + x];
            let neighbor_avg = (left + right + up + down) * 0.25;
            let gradient = (center - left)
                .abs()
                .max((center - right).abs())
                .max((center - up).abs())
                .max((center - down).abs());
            let brightness = smoothstep(threshold, 1.08, center);
            let edge = smoothstep(0.025, 0.22, gradient);
            let darker_surround = smoothstep(0.015, 0.36, center - neighbor_avg);

            edge_mask[idx] = brightness * (edge * 0.72 + darker_surround * 0.28).clamp(0.0, 1.0);
        }
    }

    let radius = ((amount * 16.0).round() as usize).clamp(2, 14);
    let local = box_blur(&edge_mask, width, height, radius);
    let wide = box_blur(
        &edge_mask,
        width,
        height,
        radius.saturating_mul(2).clamp(4, 28),
    );

    local
        .iter()
        .zip(wide.iter())
        .map(|(local, wide)| local * 0.76 + wide * 0.24)
        .collect()
}

fn box_blur(source: &[f32], width: usize, height: usize, radius: usize) -> Vec<f32> {
    let mut horizontal = vec![0.0; source.len()];
    let mut blurred = vec![0.0; source.len()];

    for y in 0..height {
        for x in 0..width {
            let x_start = x.saturating_sub(radius);
            let x_end = (x + radius).min(width - 1);
            let mut total = 0.0;
            for sample_x in x_start..=x_end {
                total += source[y * width + sample_x];
            }
            horizontal[y * width + x] = total / (x_end - x_start + 1) as f32;
        }
    }

    for y in 0..height {
        let y_start = y.saturating_sub(radius);
        let y_end = (y + radius).min(height - 1);
        for x in 0..width {
            let mut total = 0.0;
            for sample_y in y_start..=y_end {
                total += horizontal[sample_y * width + x];
            }
            blurred[y * width + x] = total / (y_end - y_start + 1) as f32;
        }
    }

    blurred
}

fn add_halation(color: &mut [f32; 3], mask: f32, recipe: &FilmRecipe, options: FilmOptions) {
    let glow = mask * options.halation * options.strength;
    if glow <= 0.0 {
        return;
    }

    for (channel, value) in color.iter_mut().enumerate() {
        let tint = recipe.halation.color[channel] * glow;
        *value = screen_blend(*value, tint);
    }
}

fn add_grain(color: &mut [f32; 3], x: u32, y: u32, options: FilmOptions, response: GrainResponse) {
    let lum = luminance(*color).clamp(0.0, 1.0);
    let shadow_weight = 1.0 - smoothstep(0.08, 0.46, lum);
    let highlight_weight = smoothstep(0.58, 1.0, lum);
    let midtone_weight = (1.0 - shadow_weight).min(1.0 - highlight_weight).max(0.0);
    let density = response.shadows * shadow_weight
        + response.midtones * midtone_weight
        + response.highlights * highlight_weight;
    let amount = options.grain * options.strength * density * 0.055;
    if amount <= 0.0 {
        return;
    }

    let grain_size = mix(options.grain_size, response.size_bias, 0.45);
    let luma_noise = multiscale_noise(x, y, options.seed, 0, grain_size);
    for (channel, value) in color.iter_mut().enumerate() {
        let chroma_noise = multiscale_noise(x, y, options.seed, channel as u32 + 1, grain_size);
        let grain_value = mix(luma_noise, chroma_noise, response.chroma);
        *value += grain_value * amount * (0.58 + lum * 0.42);
    }
}

fn multiscale_noise(x: u32, y: u32, seed: u32, channel: u32, grain_size: f32) -> f32 {
    let x = x as f32;
    let y = y as f32;
    let frequency = mix(1.55, 0.28, grain_size);
    let fine = value_noise(x * frequency, y * frequency, seed, channel);
    let medium = value_noise(
        x * frequency * 0.46,
        y * frequency * 0.46,
        seed,
        channel + 19,
    );
    let coarse = value_noise(
        x * frequency * 0.18,
        y * frequency * 0.18,
        seed,
        channel + 37,
    );

    fine * 0.58 + medium * 0.31 + coarse * 0.11
}

fn value_noise(x: f32, y: f32, seed: u32, channel: u32) -> f32 {
    let x0 = x.floor() as i32;
    let y0 = y.floor() as i32;
    let x1 = x0 + 1;
    let y1 = y0 + 1;
    let sx = smooth_fraction(x - x0 as f32);
    let sy = smooth_fraction(y - y0 as f32);

    let n00 = hash_to_noise(x0, y0, seed, channel);
    let n10 = hash_to_noise(x1, y0, seed, channel);
    let n01 = hash_to_noise(x0, y1, seed, channel);
    let n11 = hash_to_noise(x1, y1, seed, channel);

    mix(mix(n00, n10, sx), mix(n01, n11, sx), sy)
}

fn hash_to_noise(x: i32, y: i32, seed: u32, channel: u32) -> f32 {
    let mut hash = (x as u32).wrapping_mul(0x8da6_b343)
        ^ (y as u32).wrapping_mul(0xd816_3841)
        ^ seed.wrapping_mul(0xcb1a_b31f)
        ^ channel.wrapping_mul(0x1656_67b1);
    hash ^= hash >> 16;
    hash = hash.wrapping_mul(0x7feb_352d);
    hash ^= hash >> 15;
    hash = hash.wrapping_mul(0x846c_a68b);
    hash ^= hash >> 16;

    (hash as f32 / u32::MAX as f32) * 2.0 - 1.0
}

fn vignette_factor(x: u32, y: u32, width: u32, height: u32, amount: f32, strength: f32) -> f32 {
    if amount <= 0.0 {
        return 1.0;
    }

    let nx = ((x as f32 + 0.5) / width as f32) * 2.0 - 1.0;
    let ny = ((y as f32 + 0.5) / height as f32) * 2.0 - 1.0;
    let radius = (nx * nx + ny * ny).sqrt();
    let edge = smoothstep(0.42, 1.28, radius);

    1.0 - amount * strength * 0.55 * edge
}

fn apply_contrast(value: f32, contrast: f32) -> f32 {
    let pivot = 0.18;
    ((value - pivot) * contrast + pivot).max(0.0)
}

fn adjust_saturation(color: [f32; 3], saturation: f32) -> [f32; 3] {
    let gray = luminance(color);
    [
        mix(gray, color[0], saturation),
        mix(gray, color[1], saturation),
        mix(gray, color[2], saturation),
    ]
}

fn desaturate_highlights(
    color: [f32; 3],
    highlight_desaturation: f32,
    options: FilmOptions,
) -> [f32; 3] {
    if highlight_desaturation <= 0.0 {
        return color;
    }

    let weight =
        smoothstep(0.62, 1.0, luminance(color)) * highlight_desaturation * options.strength;
    adjust_saturation(color, 1.0 - weight.clamp(0.0, 0.85))
}

fn apply_hue_sectors(mut color: [f32; 3], sectors: &[HueSectorRecipe], strength: f32) -> [f32; 3] {
    if sectors.is_empty() || strength <= 0.0 {
        return color;
    }

    for sector in sectors {
        let Some((hue, saturation, lightness, scale)) = rgb_to_hsl_for_shaping(color) else {
            continue;
        };
        if saturation <= f32::EPSILON {
            continue;
        }

        let weight = hue_sector_weight(hue, *sector) * sector.amount * strength;
        if weight <= 0.0 {
            continue;
        }

        let shaped = hsl_to_rgb(
            wrap_degrees(hue + sector.hue_shift_degrees * weight),
            (saturation * mix(1.0, sector.saturation_scale, weight)).clamp(0.0, 1.0),
            (lightness * mix(1.0, sector.luminance_scale, weight)).clamp(0.0, 1.0),
        );
        color = [shaped[0] * scale, shaped[1] * scale, shaped[2] * scale];
    }

    color
}

fn rgb_to_hsl_for_shaping(color: [f32; 3]) -> Option<(f32, f32, f32, f32)> {
    let positive = [color[0].max(0.0), color[1].max(0.0), color[2].max(0.0)];
    let peak = positive[0].max(positive[1]).max(positive[2]);
    if peak <= f32::EPSILON {
        return None;
    }

    let scale = peak.max(1.0);
    let red = positive[0] / scale;
    let green = positive[1] / scale;
    let blue = positive[2] / scale;
    let max = red.max(green).max(blue);
    let min = red.min(green).min(blue);
    let chroma = max - min;
    let lightness = (max + min) * 0.5;

    if chroma <= f32::EPSILON {
        return Some((0.0, 0.0, lightness, scale));
    }

    let saturation = if lightness <= 0.5 {
        chroma / (max + min)
    } else {
        chroma / (2.0 - max - min)
    };

    let hue = if (max - red).abs() <= f32::EPSILON {
        60.0 * ((green - blue) / chroma).rem_euclid(6.0)
    } else if (max - green).abs() <= f32::EPSILON {
        60.0 * ((blue - red) / chroma + 2.0)
    } else {
        60.0 * ((red - green) / chroma + 4.0)
    };

    Some((
        wrap_degrees(hue),
        saturation.clamp(0.0, 1.0),
        lightness,
        scale,
    ))
}

fn hsl_to_rgb(hue_degrees: f32, saturation: f32, lightness: f32) -> [f32; 3] {
    let hue = wrap_degrees(hue_degrees) / 360.0;
    let saturation = saturation.clamp(0.0, 1.0);
    let lightness = lightness.clamp(0.0, 1.0);
    if saturation <= f32::EPSILON {
        return [lightness, lightness, lightness];
    }

    let q = if lightness < 0.5 {
        lightness * (1.0 + saturation)
    } else {
        lightness + saturation - lightness * saturation
    };
    let p = 2.0 * lightness - q;

    [
        hue_to_rgb(p, q, hue + 1.0 / 3.0),
        hue_to_rgb(p, q, hue),
        hue_to_rgb(p, q, hue - 1.0 / 3.0),
    ]
}

fn hue_to_rgb(p: f32, q: f32, mut hue: f32) -> f32 {
    if hue < 0.0 {
        hue += 1.0;
    } else if hue > 1.0 {
        hue -= 1.0;
    }

    if hue < 1.0 / 6.0 {
        p + (q - p) * 6.0 * hue
    } else if hue < 0.5 {
        q
    } else if hue < 2.0 / 3.0 {
        p + (q - p) * (2.0 / 3.0 - hue) * 6.0
    } else {
        p
    }
}

fn hue_sector_weight(hue: f32, sector: HueSectorRecipe) -> f32 {
    if sector.width_degrees >= 360.0 {
        return 1.0;
    }
    if sector.width_degrees <= 0.0 {
        return 0.0;
    }

    let half_width = sector.width_degrees * 0.5;
    let distance = hue_distance_degrees(hue, sector.center_degrees);
    let softness = sector.softness.clamp(0.0, 1.0);
    if softness <= f32::EPSILON {
        return if distance <= half_width { 1.0 } else { 0.0 };
    }

    let inner_width = half_width * (1.0 - softness);
    if distance <= inner_width {
        1.0
    } else if distance >= half_width {
        0.0
    } else {
        1.0 - smoothstep(inner_width, half_width, distance)
    }
}

fn hue_distance_degrees(a: f32, b: f32) -> f32 {
    let difference = (wrap_degrees(a) - wrap_degrees(b)).abs();
    difference.min(360.0 - difference)
}

fn wrap_degrees(degrees: f32) -> f32 {
    degrees.rem_euclid(360.0)
}

fn mix_color(a: [f32; 3], b: [f32; 3], amount: f32) -> [f32; 3] {
    [
        mix(a[0], b[0], amount),
        mix(a[1], b[1], amount),
        mix(a[2], b[2], amount),
    ]
}

fn luminance(color: [f32; 3]) -> f32 {
    color[0] * 0.2126 + color[1] * 0.7152 + color[2] * 0.0722
}

fn srgb_to_linear_value(value: f32) -> f32 {
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb_value(value: f32) -> f32 {
    let value = value.clamp(0.0, 1.0);
    if value <= 0.003_130_8 {
        value * 12.92
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    }
}

fn linear_to_u8(value: f32, lookup: &[u8; OUTPUT_LUT_LEN]) -> u8 {
    let index = (value.clamp(0.0, 1.0) * OUTPUT_LUT_STEPS as f32).round() as usize;
    lookup[index]
}

fn screen_blend(base: f32, add: f32) -> f32 {
    let base = base.clamp(0.0, 1.0);
    1.0 - (1.0 - base) * (1.0 - add.clamp(0.0, 1.0))
}

fn smoothstep(edge0: f32, edge1: f32, value: f32) -> f32 {
    let t = ((value - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn smooth_fraction(value: f32) -> f32 {
    let t = value.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn mix(a: f32, b: f32, amount: f32) -> f32 {
    a + (b - a) * amount
}

fn to_u8(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn clamp_unit_or(value: f32, fallback: f32) -> f32 {
    clamp_or(value, fallback, 0.0, 1.0)
}

fn clamp_or(value: f32, fallback: f32, min: f32, max: f32) -> f32 {
    if value.is_finite() {
        value.clamp(min, max)
    } else {
        fallback
    }
}

fn validate_options(options: &FilmOptions, prefix: &str) -> Result<(), RecipeError> {
    validate_range(&format!("{prefix}.strength"), options.strength, 0.0, 1.0)?;
    validate_range(
        &format!("{prefix}.exposure_stops"),
        options.exposure_stops,
        -3.0,
        3.0,
    )?;
    validate_range(&format!("{prefix}.contrast"), options.contrast, 0.25, 2.5)?;
    validate_range(&format!("{prefix}.shadows"), options.shadows, -1.0, 1.0)?;
    validate_range(
        &format!("{prefix}.highlight_rolloff"),
        options.highlight_rolloff,
        0.0,
        2.0,
    )?;
    validate_range(&format!("{prefix}.grain"), options.grain, 0.0, 1.0)?;
    validate_range(
        &format!("{prefix}.grain_size"),
        options.grain_size,
        0.0,
        1.0,
    )?;
    validate_range(&format!("{prefix}.halation"), options.halation, 0.0, 1.0)?;
    validate_range(&format!("{prefix}.vignette"), options.vignette, 0.0, 1.0)?;
    Ok(())
}

fn validate_array(name: &str, values: [f32; 3], min: f32, max: f32) -> Result<(), RecipeError> {
    for (index, value) in values.iter().enumerate() {
        validate_range(&format!("{name}[{index}]"), *value, min, max)?;
    }
    Ok(())
}

fn validate_range(name: &str, value: f32, min: f32, max: f32) -> Result<(), RecipeError> {
    if !value.is_finite() {
        return Err(RecipeError::Invalid(format!("{name} must be finite")));
    }
    if !(min..=max).contains(&value) {
        return Err(RecipeError::Invalid(format!(
            "{name} must be between {min} and {max}; got {value}"
        )));
    }
    Ok(())
}

fn default_hue_sector_softness() -> f32 {
    0.5
}

fn default_hue_sector_amount() -> f32 {
    1.0
}

fn default_hue_sector_scale() -> f32 {
    1.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgb};

    #[test]
    fn preserves_dimensions() {
        let image = sample_image();
        let recipe = sample_recipe();
        let rendered = process_image(&image, &recipe, recipe.default_options());

        assert_eq!(rendered.dimensions(), (8, 6));
    }

    #[test]
    fn same_seed_is_deterministic() {
        let image = sample_image();
        let recipe = sample_recipe();
        let options = FilmOptions {
            seed: 99,
            ..recipe.default_options()
        };

        let first = process_image(&image, &recipe, options);
        let second = process_image(&image, &recipe, options);

        assert_eq!(first.as_raw(), second.as_raw());
    }

    #[test]
    fn different_seed_changes_grain() {
        let image = sample_image();
        let recipe = sample_recipe();
        let first = process_image(
            &image,
            &recipe,
            FilmOptions {
                grain: 1.0,
                seed: 1,
                ..recipe.default_options()
            },
        );
        let second = process_image(
            &image,
            &recipe,
            FilmOptions {
                grain: 1.0,
                seed: 2,
                ..recipe.default_options()
            },
        );

        assert_ne!(first.as_raw(), second.as_raw());
    }

    #[test]
    fn grain_size_changes_texture() {
        let image = sample_image();
        let recipe = sample_recipe();
        let fine = process_image(
            &image,
            &recipe,
            FilmOptions {
                grain: 1.0,
                grain_size: 0.05,
                seed: 1,
                ..recipe.default_options()
            },
        );
        let coarse = process_image(
            &image,
            &recipe,
            FilmOptions {
                grain: 1.0,
                grain_size: 0.95,
                seed: 1,
                ..recipe.default_options()
            },
        );

        assert_ne!(fine.as_raw(), coarse.as_raw());
    }

    #[test]
    fn portra_400_35mm_is_distinct_from_hp5_plus_400() {
        let image = sample_image();
        let portra_recipe = builtin_recipe("portra-400-35mm").expect("portra recipe");
        let hp5_recipe = builtin_recipe("ilford-hp5-plus-400").expect("hp5 recipe");
        let portra_like = process_image(
            &image,
            &portra_recipe,
            FilmOptions {
                seed: 3,
                ..portra_recipe.default_options()
            },
        );
        let hp5_like = process_image(
            &image,
            &hp5_recipe,
            FilmOptions {
                seed: 3,
                ..hp5_recipe.default_options()
            },
        );

        assert_ne!(portra_like.as_raw(), hp5_like.as_raw());
    }

    #[test]
    fn kodak_gold_200_is_distinct_from_portra() {
        let image = sample_image();
        let portra_recipe = builtin_recipe("portra-400-35mm").expect("portra recipe");
        let gold_recipe = builtin_recipe("kodak-gold-200").expect("gold recipe");
        let portra_like = process_image(
            &image,
            &portra_recipe,
            FilmOptions {
                seed: 3,
                ..portra_recipe.default_options()
            },
        );
        let gold_like = process_image(
            &image,
            &gold_recipe,
            FilmOptions {
                seed: 3,
                ..gold_recipe.default_options()
            },
        );

        assert_ne!(portra_like.as_raw(), gold_like.as_raw());
    }

    #[test]
    fn kodak_ektar_100_is_distinct_from_gold_200() {
        let image = sample_image();
        let gold_recipe = builtin_recipe("kodak-gold-200").expect("gold recipe");
        let ektar_recipe = builtin_recipe("kodak-ektar-100").expect("ektar recipe");
        let gold_like = process_image(
            &image,
            &gold_recipe,
            FilmOptions {
                seed: 3,
                ..gold_recipe.default_options()
            },
        );
        let ektar_like = process_image(
            &image,
            &ektar_recipe,
            FilmOptions {
                seed: 3,
                ..ektar_recipe.default_options()
            },
        );

        assert_ne!(gold_like.as_raw(), ektar_like.as_raw());
    }

    #[test]
    fn cinestill_800t_is_distinct_from_portra() {
        let image = sample_image();
        let portra_recipe = builtin_recipe("portra-400-35mm").expect("portra recipe");
        let cinestill_recipe = builtin_recipe("cinestill-800t").expect("cinestill recipe");
        let portra_like = process_image(
            &image,
            &portra_recipe,
            FilmOptions {
                seed: 3,
                ..portra_recipe.default_options()
            },
        );
        let cinestill_like = process_image(
            &image,
            &cinestill_recipe,
            FilmOptions {
                seed: 3,
                ..cinestill_recipe.default_options()
            },
        );

        assert_ne!(portra_like.as_raw(), cinestill_like.as_raw());
    }

    #[test]
    fn kodak_tri_x_400_is_distinct_from_hp5_plus_400() {
        let image = sample_image();
        let hp5_recipe = builtin_recipe("ilford-hp5-plus-400").expect("hp5 recipe");
        let tri_x_recipe = builtin_recipe("kodak-tri-x-400").expect("tri-x recipe");
        let hp5_like = process_image(
            &image,
            &hp5_recipe,
            FilmOptions {
                seed: 3,
                ..hp5_recipe.default_options()
            },
        );
        let tri_x_like = process_image(
            &image,
            &tri_x_recipe,
            FilmOptions {
                seed: 3,
                ..tri_x_recipe.default_options()
            },
        );

        assert_ne!(hp5_like.as_raw(), tri_x_like.as_raw());
    }

    #[test]
    fn hue_sectors_target_matching_hues() {
        let image = DynamicImage::ImageRgb8(ImageBuffer::from_fn(3, 1, |x, _| match x {
            0 => Rgb([255, 0, 0]),
            1 => Rgb([0, 255, 0]),
            _ => Rgb([0, 0, 255]),
        }));
        let mut recipe = builtin_recipe("portra-400-35mm").expect("portra recipe");
        recipe.color.saturation = 1.0;
        recipe.color.highlight_desaturation = 0.0;
        recipe.color.channel_bias = [1.0, 1.0, 1.0];
        recipe.color.shadow_tint = [0.0, 0.0, 0.0];
        recipe.color.highlight_tint = [0.0, 0.0, 0.0];
        recipe.color.hue_sectors = vec![HueSectorRecipe {
            center_degrees: 120.0,
            width_degrees: 70.0,
            softness: 0.0,
            amount: 1.0,
            hue_shift_degrees: 0.0,
            saturation_scale: 0.0,
            luminance_scale: 1.0,
        }];
        let mut baseline_recipe = recipe.clone();
        baseline_recipe.color.hue_sectors.clear();
        let options = FilmOptions {
            strength: 1.0,
            grain: 0.0,
            halation: 0.0,
            vignette: 0.0,
            ..recipe.default_options()
        };

        let baseline = process_image(&image, &baseline_recipe, options);
        let shaped = process_image(&image, &recipe, options);

        assert_eq!(&baseline.as_raw()[0..3], &shaped.as_raw()[0..3]);
        assert_ne!(&baseline.as_raw()[3..6], &shaped.as_raw()[3..6]);
        assert_eq!(&baseline.as_raw()[6..9], &shaped.as_raw()[6..9]);
    }

    #[test]
    fn invalid_hue_sector_is_rejected() {
        let mut recipe = sample_recipe();
        recipe.color.hue_sectors = vec![HueSectorRecipe {
            center_degrees: 120.0,
            width_degrees: 361.0,
            softness: 0.5,
            amount: 1.0,
            hue_shift_degrees: 0.0,
            saturation_scale: 1.0,
            luminance_scale: 1.0,
        }];

        let error = recipe.validate().expect_err("invalid hue sector");

        assert!(
            error
                .to_string()
                .contains("color.hue_sectors[0].width_degrees")
        );
    }

    #[test]
    fn built_in_recipe_lookup_requires_exact_ids() {
        assert!(builtin_recipe("portra-400-35mm").is_some());
        assert!(builtin_recipe("kodak-gold-200").is_some());
        assert!(builtin_recipe("kodak-ektar-100").is_some());
        assert!(builtin_recipe("ilford-hp5-plus-400").is_some());
        assert!(builtin_recipe("kodak-tri-x-400").is_some());
        assert!(builtin_recipe("cinestill-800t").is_some());
        assert!(builtin_recipe("portra400").is_none());
        assert!(builtin_recipe("gold-200").is_none());
        assert!(builtin_recipe("ektar").is_none());
        assert!(builtin_recipe("hp5").is_none());
        assert!(builtin_recipe("tri-x").is_none());
        assert!(builtin_recipe("800t").is_none());
        assert!(builtin_recipe("clean-negative").is_none());
        assert!(builtin_recipe("mono").is_none());
    }

    #[test]
    fn recipe_json_rejects_removed_alias_metadata() {
        let json = builtin_recipe_json("portra-400-35mm")
            .expect("portra json")
            .replacen(
                "\"defaults\"",
                "\"aliases\": [\"clean\"],\n  \"defaults\"",
                1,
            );

        let error = FilmRecipe::from_json_str(&json).expect_err("removed aliases are invalid");

        assert!(error.to_string().contains("aliases"));
    }

    fn sample_image() -> DynamicImage {
        let image = ImageBuffer::from_fn(8, 6, |x, y| {
            Rgb([
                (x * 28 + 20) as u8,
                (y * 34 + 30) as u8,
                ((x + y) * 18 + 40) as u8,
            ])
        });
        DynamicImage::ImageRgb8(image)
    }

    fn sample_recipe() -> FilmRecipe {
        builtin_recipe("portra-400-35mm").expect("portra recipe")
    }
}
