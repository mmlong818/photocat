/**
 * Batch export.
 *
 * The app could overwrite an original or save one edited copy, and nothing in
 * between: no way to turn a folder of 40-megapixel RAWs into web-sized JPEGs,
 * strip location data before sharing, or produce the same three sizes every
 * time. This module adds that layer — a saved recipe applied to a selection,
 * writing new files into a folder the user picks and never touching the
 * originals.
 *
 * The work runs on a background task that reports progress per file and stops
 * on request. A file that fails to decode or write is counted and skipped, so
 * one unreadable image cannot abandon a batch of a thousand.
 *
 * Decoding goes through `t_image::decode_source_image`, which covers every
 * format the app can display. Note that RAW, HEIC, JXL, TIFF, AVIF and the
 * FFmpeg-backed formats decode through preview pipelines capped at 4096 px on
 * the long edge, so exports of those cannot exceed that.
 */
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::t_image;

/// Emit at most this many progress events per second; a batch of thousands of
/// small files would otherwise spend real time on IPC.
const PROGRESS_INTERVAL_MS: u128 = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Jpeg,
    Png,
    Webp,
}

impl ExportFormat {
    fn extension(self) -> &'static str {
        match self {
            ExportFormat::Jpeg => "jpg",
            ExportFormat::Png => "png",
            ExportFormat::Webp => "webp",
        }
    }

    fn keeps_metadata(self) -> bool {
        // Only these two carry an EXIF block the app knows how to write.
        matches!(self, ExportFormat::Jpeg | ExportFormat::Webp)
    }
}

/// What to do when the destination already holds a file of the same name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictPolicy {
    /// Append (1), (2), … so nothing is lost.
    KeepBoth,
    /// Overwrite the existing file.
    Replace,
    /// Leave the existing file alone and count the source as skipped.
    Skip,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportPreset {
    pub format: ExportFormat,
    /// 1-100, JPEG only. PNG is lossless and the bundled WebP encoder has no
    /// quality control.
    pub quality: u8,
    /// Longest edge in pixels. `None` or 0 exports at the decoded size.
    /// Images already smaller than this are never enlarged.
    pub max_edge: Option<u32>,
    /// Carry EXIF across. Turning this off is the way to drop camera
    /// information and GPS coordinates before sharing.
    pub keep_metadata: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRequest {
    /// Absolute source paths, in the order the user selected them.
    pub files: Vec<String>,
    pub destination: String,
    pub preset: ExportPreset,
    pub conflict: ConflictPolicy,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportProgress {
    pub current_name: Option<String>,
    pub processed: usize,
    pub total: usize,
    pub exported: usize,
    pub skipped: usize,
    pub failed: usize,
    pub cancelled: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub total: usize,
    pub exported: usize,
    pub skipped: usize,
    pub failed: usize,
    pub cancelled: bool,
    /// First few failures, so the UI can say what went wrong rather than only
    /// how many.
    pub errors: Vec<String>,
}

#[derive(Default)]
pub struct ExportState {
    pub cancelled: AtomicBool,
    pub running: AtomicBool,
}

#[derive(Default)]
pub struct ExportCancellation(pub Arc<ExportState>);

/// Longest-edge fit. Returns `None` when the image already fits, so callers
/// can skip a needless resample.
fn fit_to_max_edge(width: u32, height: u32, max_edge: u32) -> Option<(u32, u32)> {
    let longest = width.max(height);
    if max_edge == 0 || longest <= max_edge {
        return None;
    }
    let scale = max_edge as f64 / longest as f64;
    let target_width = ((width as f64 * scale).round() as u32).max(1);
    let target_height = ((height as f64 * scale).round() as u32).max(1);
    Some((target_width, target_height))
}

/// `name.jpg`, `name(1).jpg`, `name(2).jpg`, … Mirrors the naming the app
/// already uses for copy and move conflicts.
fn unique_path(candidate: PathBuf) -> PathBuf {
    if !candidate.exists() {
        return candidate;
    }
    let parent = candidate.parent().unwrap_or_else(|| Path::new(""));
    let stem = candidate
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let extension = candidate
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let mut counter = 1_u32;
    loop {
        let name = if extension.is_empty() {
            format!("{}({})", stem, counter)
        } else {
            format!("{}({}).{}", stem, counter, extension)
        };
        let next = parent.join(name);
        if !next.exists() {
            return next;
        }
        counter += 1;
    }
}

/// Resolve where one source lands. `Ok(None)` means the policy says skip.
fn destination_for(
    source: &Path,
    destination_dir: &str,
    extension: &str,
    conflict: ConflictPolicy,
) -> Result<Option<PathBuf>, String> {
    let stem = source
        .file_stem()
        .ok_or_else(|| format!("File has no name: {}", source.display()))?;
    let candidate = Path::new(destination_dir).join(format!(
        "{}.{}",
        stem.to_string_lossy(),
        extension
    ));

    if !candidate.exists() {
        return Ok(Some(candidate));
    }
    match conflict {
        ConflictPolicy::Skip => Ok(None),
        ConflictPolicy::Replace => Ok(Some(candidate)),
        ConflictPolicy::KeepBoth => Ok(Some(unique_path(candidate))),
    }
}

fn encode_to(
    img: &image::DynamicImage,
    format: ExportFormat,
    quality: u8,
    path: &Path,
) -> Result<(), String> {
    match format {
        ExportFormat::Jpeg => {
            // JPEG has no alpha; flatten first or the encoder rejects RGBA.
            let rgb = image::DynamicImage::ImageRgb8(img.to_rgb8());
            let file = std::fs::File::create(path)
                .map_err(|e| format!("Failed to create '{}': {}", path.display(), e))?;
            let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(
                std::io::BufWriter::new(file),
                quality.clamp(1, 100),
            );
            encoder
                .encode_image(&rgb)
                .map_err(|e| format!("Failed to encode JPEG: {}", e))
        }
        ExportFormat::Png => img
            .save_with_format(path, image::ImageFormat::Png)
            .map_err(|e| format!("Failed to encode PNG: {}", e)),
        ExportFormat::Webp => img
            .save_with_format(path, image::ImageFormat::WebP)
            .map_err(|e| format!("Failed to encode WebP: {}", e)),
    }
}

/// Export one file. Returns the path written, or `None` when the conflict
/// policy said to skip it.
async fn export_one(
    source_path: &str,
    destination_dir: &str,
    preset: &ExportPreset,
    conflict: ConflictPolicy,
) -> Result<Option<PathBuf>, String> {
    let source = Path::new(source_path);
    if !source.is_file() {
        return Err(format!("File not found: {}", source_path));
    }

    let Some(dest) = destination_for(source, destination_dir, preset.format.extension(), conflict)?
    else {
        return Ok(None);
    };

    let orientation = t_image::get_image_orientation(source_path);
    let mut img = t_image::decode_source_image(source_path, orientation).await?;

    if let Some(max_edge) = preset.max_edge {
        if let Some((width, height)) = fit_to_max_edge(img.width(), img.height(), max_edge) {
            img = img.resize_exact(width, height, image::imageops::FilterType::Lanczos3);
        }
    }

    encode_to(&img, preset.format, preset.quality, &dest)?;

    if preset.keep_metadata && preset.format.keeps_metadata() {
        // Losing metadata is not worth losing the exported image over.
        if let Err(e) = t_image::copy_metadata_to_output(source, &dest) {
            eprintln!("export: metadata copy failed for {}: {}", source_path, e);
        }
    }

    Ok(Some(dest))
}

/// Run a whole export. `report_progress` is called as files complete and
/// `is_cancelled` is polled between them, keeping this free of Tauri types.
pub async fn export_files<F, C>(
    request: ExportRequest,
    mut report_progress: F,
    is_cancelled: C,
) -> Result<ExportResult, String>
where
    F: FnMut(ExportProgress),
    C: Fn() -> bool,
{
    let destination = Path::new(&request.destination);
    if !destination.is_dir() {
        return Err(format!(
            "Destination folder does not exist: {}",
            request.destination
        ));
    }

    let total = request.files.len();
    let mut exported = 0_usize;
    let mut skipped = 0_usize;
    let mut failed = 0_usize;
    let mut errors: Vec<String> = Vec::new();
    let mut last_emit = std::time::Instant::now();

    report_progress(ExportProgress {
        current_name: None,
        processed: 0,
        total,
        exported: 0,
        skipped: 0,
        failed: 0,
        cancelled: false,
    });

    for (index, source_path) in request.files.iter().enumerate() {
        if is_cancelled() {
            report_progress(ExportProgress {
                current_name: None,
                processed: index,
                total,
                exported,
                skipped,
                failed,
                cancelled: true,
            });
            return Ok(ExportResult {
                total,
                exported,
                skipped,
                failed,
                cancelled: true,
                errors,
            });
        }

        let name = Path::new(source_path)
            .file_name()
            .map(|value| value.to_string_lossy().to_string());

        match export_one(
            source_path,
            &request.destination,
            &request.preset,
            request.conflict,
        )
        .await
        {
            Ok(Some(_)) => exported += 1,
            Ok(None) => skipped += 1,
            Err(e) => {
                failed += 1;
                if errors.len() < 10 {
                    errors.push(e);
                }
            }
        }

        let processed = index + 1;
        let is_last = processed == total;
        if is_last || last_emit.elapsed().as_millis() >= PROGRESS_INTERVAL_MS {
            last_emit = std::time::Instant::now();
            report_progress(ExportProgress {
                current_name: name,
                processed,
                total,
                exported,
                skipped,
                failed,
                cancelled: false,
            });
        }
    }

    Ok(ExportResult {
        total,
        exported,
        skipped,
        failed,
        cancelled: false,
        errors,
    })
}

/// Guards against two exports running at once and carries the cancel flag.
pub fn begin(state: &ExportState) -> Result<(), String> {
    if state.running.swap(true, Ordering::SeqCst) {
        return Err("An export is already in progress".to_string());
    }
    state.cancelled.store(false, Ordering::SeqCst);
    Ok(())
}

pub fn finish(state: &ExportState) {
    state.running.store(false, Ordering::SeqCst);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn max_edge_only_shrinks_and_keeps_aspect() {
        // Landscape 4000x3000 into 2000 => 2000x1500.
        assert_eq!(fit_to_max_edge(4000, 3000, 2000), Some((2000, 1500)));
        // Portrait limits on the tall side.
        assert_eq!(fit_to_max_edge(3000, 4000, 2000), Some((1500, 2000)));
        // Already small enough: no resample.
        assert_eq!(fit_to_max_edge(1200, 800, 2000), None);
        // Exactly on the limit counts as fitting.
        assert_eq!(fit_to_max_edge(2000, 1000, 2000), None);
        // Zero disables the limit.
        assert_eq!(fit_to_max_edge(8000, 6000, 0), None);
        // Never rounds an edge away to nothing.
        assert_eq!(fit_to_max_edge(10000, 3, 100), Some((100, 1)));
    }

    #[test]
    fn conflict_policy_picks_a_destination() {
        let dir = std::env::temp_dir().join(format!("lap-export-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let source = dir.join("photo.cr2");
        std::fs::write(&source, b"x").unwrap();
        let dir_str = dir.to_string_lossy().to_string();

        // Nothing there yet: the extension follows the chosen format.
        let first = destination_for(&source, &dir_str, "jpg", ConflictPolicy::KeepBoth)
            .unwrap()
            .unwrap();
        assert_eq!(first.file_name().unwrap(), "photo.jpg");

        std::fs::write(&first, b"x").unwrap();

        // Keep both: the original output survives.
        let second = destination_for(&source, &dir_str, "jpg", ConflictPolicy::KeepBoth)
            .unwrap()
            .unwrap();
        assert_eq!(second.file_name().unwrap(), "photo(1).jpg");

        // Replace: same path, to be overwritten.
        let replaced = destination_for(&source, &dir_str, "jpg", ConflictPolicy::Replace)
            .unwrap()
            .unwrap();
        assert_eq!(replaced, first);

        // Skip: no destination at all.
        assert!(destination_for(&source, &dir_str, "jpg", ConflictPolicy::Skip)
            .unwrap()
            .is_none());

        std::fs::remove_dir_all(&dir).ok();
    }
}
