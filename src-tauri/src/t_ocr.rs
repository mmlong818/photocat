/**
 * Text recognition inside images.
 *
 * Most people's libraries are full of screenshots — receipts, tickets, chat
 * threads, slides, notes — and none of it is findable. Semantic search knows
 * a picture contains "a document"; it cannot tell you which one says
 * "invoice". Reading the words puts that whole half of a library back within
 * reach of the search box.
 *
 * The recogniser is the one the operating system already ships, so nothing is
 * downloaded and nothing is added to the installer. That does mean coverage
 * follows the languages installed on the machine, and a platform without a
 * system recogniser reports the feature as unavailable rather than pretending.
 *
 * Recognised text is stored per file and joined into the ordinary search, so
 * typing a word finds the screenshot containing it with no separate mode.
 */

/// True for scripts written without spaces between characters.
fn is_ideographic(c: char) -> bool {
    matches!(c as u32,
        0x3000..=0x303F |   // CJK punctuation
        0x3040..=0x30FF |   // kana
        0x3400..=0x4DBF |   // unified ideographs extension A
        0x4E00..=0x9FFF |   // unified ideographs
        0xAC00..=0xD7AF |   // hangul syllables
        0xF900..=0xFAFF |   // compatibility ideographs
        0xFF00..=0xFFEF     // fullwidth and halfwidth forms
    )
}

/// The Windows recogniser treats every ideograph as its own word, so a line of
/// Chinese comes back with a space after every character. Left alone, a search
/// for a two-character word would never match. Drop the spaces that sit
/// between ideographs and keep the ones separating Latin words.
fn join_ideographs(line: &str) -> String {
    let chars: Vec<char> = line.chars().collect();
    let mut out = String::with_capacity(line.len());
    let mut index = 0;
    while index < chars.len() {
        let c = chars[index];
        if c == ' ' {
            let previous = out.chars().next_back();
            let next = chars[index + 1..].iter().copied().find(|&n| n != ' ');
            if let (Some(previous), Some(next)) = (previous, next) {
                if is_ideographic(previous) && is_ideographic(next) {
                    index += 1;
                    continue;
                }
            }
        }
        out.push(c);
        index += 1;
    }
    out
}

/// Why recognition cannot run, phrased for the person rather than the log.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OcrAvailability {
    Ready,
    /// The platform has no system recogniser this build can use.
    UnsupportedPlatform,
    /// Windows has the API but no OCR language is installed.
    NoLanguage,
}

#[cfg(target_os = "windows")]
mod platform {
    use super::OcrAvailability;
    use windows::Graphics::Imaging::BitmapDecoder;
    use windows::Media::Ocr::OcrEngine;
    use windows::Storage::Streams::{DataWriter, InMemoryRandomAccessStream};

    /// Windows exposes recognisers for the languages whose packs are
    /// installed. `TryCreateFromUserProfileLanguages` picks the ones the user
    /// actually reads, which is the right default: a Chinese system recognises
    /// Chinese and Latin script, an English one recognises Latin.
    fn engine() -> Result<OcrEngine, String> {
        OcrEngine::TryCreateFromUserProfileLanguages()
            .map_err(|e| format!("Failed to create the system OCR engine: {}", e))
    }

    pub fn availability() -> OcrAvailability {
        match engine() {
            Ok(_) => OcrAvailability::Ready,
            Err(_) => OcrAvailability::NoLanguage,
        }
    }

    /// Recognise every line in an encoded image and return them joined by
    /// newlines. Empty when the image holds no legible text.
    pub fn recognize_bytes(bytes: &[u8]) -> Result<String, String> {
        let engine = engine()?;

        // WinRT decodes from its own stream type, so the buffer has to be
        // handed across rather than passed by pointer.
        let stream = InMemoryRandomAccessStream::new()
            .map_err(|e| format!("Failed to create an image stream: {}", e))?;
        let writer = DataWriter::CreateDataWriter(&stream)
            .map_err(|e| format!("Failed to open the image stream: {}", e))?;
        writer
            .WriteBytes(bytes)
            .map_err(|e| format!("Failed to write the image: {}", e))?;
        writer
            .StoreAsync()
            .and_then(|op| op.get())
            .map_err(|e| format!("Failed to store the image: {}", e))?;
        writer
            .FlushAsync()
            .and_then(|op| op.get())
            .map_err(|e| format!("Failed to flush the image: {}", e))?;
        stream
            .Seek(0)
            .map_err(|e| format!("Failed to rewind the image: {}", e))?;

        let decoder = BitmapDecoder::CreateAsync(&stream)
            .and_then(|op| op.get())
            .map_err(|e| format!("Failed to decode the image: {}", e))?;
        let bitmap = decoder
            .GetSoftwareBitmapAsync()
            .and_then(|op| op.get())
            .map_err(|e| format!("Failed to read the image pixels: {}", e))?;

        let result = engine
            .RecognizeAsync(&bitmap)
            .and_then(|op| op.get())
            .map_err(|e| format!("Recognition failed: {}", e))?;

        let lines = result
            .Lines()
            .map_err(|e| format!("Failed to read the recognised lines: {}", e))?;

        let mut out = String::new();
        for line in lines {
            let text = line
                .Text()
                .map_err(|e| format!("Failed to read a recognised line: {}", e))?;
            let text = text.to_string();
            if text.trim().is_empty() {
                continue;
            }
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(&super::join_ideographs(text.trim()));
        }
        Ok(out)
    }
}

#[cfg(not(target_os = "windows"))]
mod platform {
    use super::OcrAvailability;

    pub fn availability() -> OcrAvailability {
        OcrAvailability::UnsupportedPlatform
    }

    pub fn recognize_bytes(_bytes: &[u8]) -> Result<String, String> {
        Err("Text recognition is not available on this platform yet".to_string())
    }
}

pub use platform::availability;

/// Longest edge fed to the recogniser. Screenshots are already legible at this
/// size and shrinking first keeps a 45-megapixel photo from costing seconds.
const MAX_EDGE: u32 = 2000;

/// Smallest edge worth attempting; below this any text is unreadable anyway.
const MIN_EDGE: u32 = 64;

/// Recognise the text in one file. Returns an empty string when the image is
/// legible but wordless, which is a result worth storing so the file is not
/// retried on every pass.
pub async fn recognize_file(file_path: &str, orientation: i32) -> Result<String, String> {
    let img = crate::t_image::decode_source_image(file_path, orientation).await?;
    let (width, height) = (img.width(), img.height());
    if width < MIN_EDGE || height < MIN_EDGE {
        return Ok(String::new());
    }

    let longest = width.max(height);
    let prepared = if longest > MAX_EDGE {
        let scale = MAX_EDGE as f64 / longest as f64;
        let target_width = ((width as f64 * scale).round() as u32).max(1);
        let target_height = ((height as f64 * scale).round() as u32).max(1);
        img.resize_exact(
            target_width,
            target_height,
            image::imageops::FilterType::Lanczos3,
        )
    } else {
        img
    };

    // Re-encode losslessly; the system decoder takes an encoded image, and PNG
    // avoids adding compression artefacts to text the recogniser must read.
    let mut buffer = std::io::Cursor::new(Vec::new());
    prepared
        .write_to(&mut buffer, image::ImageFormat::Png)
        .map_err(|e| format!("Failed to prepare the image for recognition: {}", e))?;

    platform::recognize_bytes(&buffer.into_inner())
}

// ---------------------------------------------------------------------------
// Storage
// ---------------------------------------------------------------------------

/// Recognised text lives in its own table rather than a column on `afiles`,
/// so the feature can be added, cleared or skipped without touching rows the
/// rest of the app depends on. A row exists once a file has been read, even
/// when the result was empty, which is what stops a wordless photo from being
/// reprocessed on every pass.
pub fn ensure_schema() -> Result<(), String> {
    let conn = crate::t_sqlite::open_conn()?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS file_texts (
            file_id     INTEGER PRIMARY KEY,
            text        TEXT NOT NULL,
            recognized_at INTEGER NOT NULL,
            FOREIGN KEY (file_id) REFERENCES afiles(id) ON DELETE CASCADE
        );",
    )
    .map_err(|e| format!("Failed to create the recognised-text table: {}", e))
}

pub fn save_text(file_id: i64, text: &str) -> Result<(), String> {
    let conn = crate::t_sqlite::open_conn()?;
    conn.execute(
        "INSERT INTO file_texts (file_id, text, recognized_at) VALUES (?1, ?2, ?3)
         ON CONFLICT(file_id) DO UPDATE SET text = excluded.text,
                                            recognized_at = excluded.recognized_at",
        rusqlite::params![file_id, text, chrono::Utc::now().timestamp_millis()],
    )
    .map(|_| ())
    .map_err(|e| format!("Failed to store recognised text: {}", e))
}

/// `(files already read, of which carried text)` for the current library.
pub fn stats() -> Result<(i64, i64), String> {
    let conn = crate::t_sqlite::open_conn()?;
    conn.query_row(
        "SELECT COUNT(*), COALESCE(SUM(CASE WHEN length(text) > 0 THEN 1 ELSE 0 END), 0)
         FROM file_texts",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .map_err(|e| format!("Failed to count recognised text: {}", e))
}

/// Which of `file_ids` have not been read yet, preserving the caller's order.
pub fn pending(file_ids: &[i64]) -> Result<Vec<i64>, String> {
    if file_ids.is_empty() {
        return Ok(Vec::new());
    }
    let conn = crate::t_sqlite::open_conn()?;
    let mut done = std::collections::HashSet::new();
    for chunk in file_ids.chunks(900) {
        let placeholders = std::iter::repeat("?")
            .take(chunk.len())
            .collect::<Vec<_>>()
            .join(",");
        let mut stmt = conn
            .prepare(&format!(
                "SELECT file_id FROM file_texts WHERE file_id IN ({})",
                placeholders
            ))
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(rusqlite::params_from_iter(chunk.iter()), |row| {
                row.get::<_, i64>(0)
            })
            .map_err(|e| e.to_string())?;
        for row in rows {
            done.insert(row.map_err(|e| e.to_string())?);
        }
    }
    Ok(file_ids
        .iter()
        .copied()
        .filter(|id| !done.contains(id))
        .collect())
}

/// Forget every recognised result for the current library.
pub fn clear_all() -> Result<(), String> {
    let conn = crate::t_sqlite::open_conn()?;
    conn.execute("DELETE FROM file_texts", [])
        .map(|_| ())
        .map_err(|e| format!("Failed to clear recognised text: {}", e))
}

// ---------------------------------------------------------------------------
// Batch recognition
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrProgress {
    pub processed: usize,
    pub total: usize,
    /// Files where words were found.
    pub with_text: usize,
    pub failed: usize,
    pub cancelled: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrResult {
    pub total: usize,
    pub processed: usize,
    pub with_text: usize,
    pub failed: usize,
    pub cancelled: bool,
    pub errors: Vec<String>,
}

#[derive(Default)]
pub struct OcrState {
    pub cancelled: std::sync::atomic::AtomicBool,
    pub running: std::sync::atomic::AtomicBool,
}

#[derive(Default)]
pub struct OcrCancellation(pub std::sync::Arc<OcrState>);

/// Progress is emitted at most this often; recognising a small screenshot
/// takes tens of milliseconds and a per-file event would flood the bridge.
const PROGRESS_INTERVAL_MS: u128 = 150;

pub fn begin(state: &OcrState) -> Result<(), String> {
    use std::sync::atomic::Ordering;
    if state.running.swap(true, Ordering::SeqCst) {
        return Err("Text recognition is already running".to_string());
    }
    state.cancelled.store(false, Ordering::SeqCst);
    Ok(())
}

pub fn finish(state: &OcrState) {
    state
        .running
        .store(false, std::sync::atomic::Ordering::SeqCst);
}

/// Read `files` and store what each one says. `(file id, path, orientation)`
/// triples come from the caller so this stays free of query concerns.
pub async fn recognize_files<F, C>(
    files: Vec<(i64, String, i32)>,
    mut report_progress: F,
    is_cancelled: C,
) -> Result<OcrResult, String>
where
    F: FnMut(OcrProgress),
    C: Fn() -> bool,
{
    if availability() != OcrAvailability::Ready {
        return Err("No system text recogniser is available".to_string());
    }
    ensure_schema()?;

    let total = files.len();
    let mut with_text = 0_usize;
    let mut failed = 0_usize;
    let mut errors: Vec<String> = Vec::new();
    let mut last_emit = std::time::Instant::now();

    report_progress(OcrProgress {
        processed: 0,
        total,
        with_text: 0,
        failed: 0,
        cancelled: false,
    });

    for (index, (file_id, path, orientation)) in files.into_iter().enumerate() {
        if is_cancelled() {
            report_progress(OcrProgress {
                processed: index,
                total,
                with_text,
                failed,
                cancelled: true,
            });
            return Ok(OcrResult {
                total,
                processed: index,
                with_text,
                failed,
                cancelled: true,
                errors,
            });
        }

        match recognize_file(&path, orientation).await {
            Ok(text) => {
                if !text.trim().is_empty() {
                    with_text += 1;
                }
                // Store even an empty result so the file is not read again.
                if let Err(e) = save_text(file_id, &text) {
                    failed += 1;
                    if errors.len() < 10 {
                        errors.push(e);
                    }
                }
            }
            Err(e) => {
                failed += 1;
                if errors.len() < 10 {
                    errors.push(format!("{}: {}", path, e));
                }
            }
        }

        let processed = index + 1;
        if processed == total || last_emit.elapsed().as_millis() >= PROGRESS_INTERVAL_MS {
            last_emit = std::time::Instant::now();
            report_progress(OcrProgress {
                processed,
                total,
                with_text,
                failed,
                cancelled: false,
            });
        }
    }

    Ok(OcrResult {
        total,
        processed: total,
        with_text,
        failed,
        cancelled: false,
        errors,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ideograph_spacing_is_normalised() {
        // What the Windows recogniser actually emits for CJK text.
        assert_eq!(join_ideographs("猫 叔 的 图"), "猫叔的图");
        // Latin words keep their separators.
        assert_eq!(join_ideographs("hello world"), "hello world");
        // A script change keeps one space on the boundary.
        assert_eq!(join_ideographs("猫 叔 的 漫 画 StoryBoard"), "猫叔的漫画 StoryBoard");
        assert_eq!(join_ideographs("Gemini 3 Pro 角 色 工 坊"), "Gemini 3 Pro 角色工坊");
        // Runs of spaces collapse only between ideographs.
        assert_eq!(join_ideographs("剧  本  改"), "剧本改");
        assert_eq!(join_ideographs(""), "");
    }

    /// Confirms the system recogniser is reachable and reads real text.
    /// Point `LAP_TEST_IMAGE` at an image containing words.
    #[test]
    fn reads_text_from_an_image() {
        if availability() != OcrAvailability::Ready {
            eprintln!("no system OCR engine available; skipping");
            return;
        }
        let Ok(path) = std::env::var("LAP_TEST_IMAGE") else {
            eprintln!("LAP_TEST_IMAGE not set; skipping");
            return;
        };
        let text = tauri::async_runtime::block_on(recognize_file(&path, 1))
            .expect("recognition should succeed");
        println!("--- recognised {} chars ---\n{}", text.chars().count(), text);
        assert!(!text.trim().is_empty(), "expected some text in {}", path);
    }
}
