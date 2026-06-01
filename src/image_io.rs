use std::{
    fs::{self, File, create_dir_all},
    io::{BufReader, BufWriter},
    path::Path,
};

use anyhow::{Context, Result};
use exif::{In, Tag};
use image::{DynamicImage, ImageReader, RgbImage, codecs::jpeg::JpegEncoder};
use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub struct LoadedImage {
    pub image: DynamicImage,
    pub metadata: InputMetadata,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct InputMetadata {
    pub orientation: Option<u32>,
    pub color_profile: ColorProfile,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(tag = "kind", content = "name")]
pub enum ColorProfile {
    #[default]
    AssumedSrgb,
    Srgb,
    EmbeddedIcc(String),
    PngIcc(String),
}

pub fn load_image(path: &Path, auto_orient: bool) -> Result<LoadedImage> {
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

pub fn read_input_metadata(path: &Path) -> InputMetadata {
    InputMetadata {
        orientation: read_exif_orientation(path),
        color_profile: detect_color_profile(path),
    }
}

pub fn save_image(image: &RgbImage, path: &Path, jpeg_quality: u8) -> Result<()> {
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

pub fn is_supported_input(path: &Path) -> bool {
    extension_matches(
        path,
        &["jpg", "jpeg", "png", "tif", "tiff", "bmp", "webp", "qoi"],
    )
}

pub fn is_within_canonical_path(path: &Path, canonical_parent: &Path) -> bool {
    path.canonicalize()
        .is_ok_and(|canonical_path| canonical_path.starts_with(canonical_parent))
}

impl InputMetadata {
    pub fn summary(&self, auto_orient: bool) -> String {
        let orientation = match (self.orientation, auto_orient) {
            (Some(orientation), true) => format!("EXIF orientation {orientation} applied"),
            (Some(orientation), false) => {
                format!("EXIF orientation {orientation} present; auto-orientation disabled")
            }
            (None, _) => "no EXIF orientation".to_string(),
        };

        format!("{orientation}; {}", self.color_profile.summary())
    }
}

impl ColorProfile {
    pub fn summary(&self) -> String {
        match self {
            ColorProfile::AssumedSrgb => "no embedded color profile; assuming sRGB".to_string(),
            ColorProfile::Srgb => "sRGB color profile".to_string(),
            ColorProfile::EmbeddedIcc(name) => format!("{name} ICC profile"),
            ColorProfile::PngIcc(name) => format!("{name} PNG ICC profile"),
        }
    }

    pub fn requires_assumption_warning(&self) -> bool {
        matches!(self, ColorProfile::EmbeddedIcc(_) | ColorProfile::PngIcc(_))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_summary_reports_disabled_auto_orientation() {
        let metadata = InputMetadata {
            orientation: Some(6),
            color_profile: ColorProfile::AssumedSrgb,
        };

        let summary = metadata.summary(false);

        assert!(summary.contains("EXIF orientation 6 present"));
        assert!(summary.contains("auto-orientation disabled"));
    }
}
