/**
 * Metadata sidecars.
 *
 * Ratings, tags and notes live in this app's database and nowhere else. That
 * is fast and keeps the originals untouched, but it also means the work of
 * organising a library cannot leave: copy the photos to another machine, or
 * open them in Lightroom or digiKam, and every star and keyword is gone. For
 * anyone with years of sorting invested, that is a reason not to start.
 *
 * This writes that work into XMP sidecar files next to the originals, in the
 * form other photo software already reads, and reads them back again. It is
 * an explicit export and import rather than a live mirror: the database stays
 * the source of truth, and a sidecar is a portable copy taken at a moment in
 * time.
 *
 * Sidecar naming follows each ecosystem's expectation, so the file is found
 * rather than ignored: RAW files get `photo.xmp`, which is what Adobe tools
 * look for, and everything else gets `photo.jpg.xmp`, which is what digiKam
 * and darktable look for and can never collide with a differently-typed
 * sibling of the same name.
 */
use quick_xml::events::Event;
use quick_xml::Reader;
use std::path::{Path, PathBuf};

/// The subset of a file's organisation that has a standard XMP home.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SidecarMetadata {
    /// 0-5, or -1 for a rejected photo, matching the Adobe convention.
    pub rating: Option<i32>,
    /// Free-text label. This app writes "Favorite"; other tools write colours.
    pub label: Option<String>,
    pub tags: Vec<String>,
    pub description: Option<String>,
}

impl SidecarMetadata {
    pub fn is_empty(&self) -> bool {
        self.rating.is_none()
            && self.label.is_none()
            && self.tags.is_empty()
            && self.description.as_deref().unwrap_or("").is_empty()
    }
}

/// The label this app writes for a favourite. Other tools show it verbatim.
pub const FAVORITE_LABEL: &str = "Favorite";

/// Where the sidecar for `file_path` belongs.
pub fn sidecar_path(file_path: &str) -> PathBuf {
    let path = Path::new(file_path);
    let is_raw = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| {
            crate::t_common::RAW_IMGS
                .iter()
                .any(|raw| raw.eq_ignore_ascii_case(ext))
        })
        .unwrap_or(false);

    if is_raw {
        // Adobe tools expect the extension to be replaced for raw files.
        path.with_extension("xmp")
    } else {
        // Appending keeps photo.jpg and photo.png distinct.
        let mut name = path.file_name().unwrap_or_default().to_os_string();
        name.push(".xmp");
        path.with_file_name(name)
    }
}

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

/// Render a standard XMP packet. Fields that are absent are left out entirely
/// rather than written empty, so a reader cannot mistake "not set" for "zero".
pub fn build(meta: &SidecarMetadata) -> String {
    let mut attrs = String::new();
    if let Some(rating) = meta.rating {
        attrs.push_str(&format!("\n   xmp:Rating=\"{}\"", rating));
    }
    if let Some(label) = meta.label.as_deref().filter(|l| !l.is_empty()) {
        attrs.push_str(&format!("\n   xmp:Label=\"{}\"", escape(label)));
    }

    let mut body = String::new();
    if !meta.tags.is_empty() {
        body.push_str("\n   <dc:subject>\n    <rdf:Bag>");
        for tag in &meta.tags {
            body.push_str(&format!("\n     <rdf:li>{}</rdf:li>", escape(tag)));
        }
        body.push_str("\n    </rdf:Bag>\n   </dc:subject>");
    }
    if let Some(description) = meta.description.as_deref().filter(|d| !d.is_empty()) {
        body.push_str(&format!(
            "\n   <dc:description>\n    <rdf:Alt>\n     <rdf:li xml:lang=\"x-default\">{}</rdf:li>\n    </rdf:Alt>\n   </dc:description>",
            escape(description)
        ));
    }

    format!(
        r#"<?xpacket begin="" id="W5M0MpCehiHzreSzNTczkc9d"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/" x:xmptk="Maoshu Photos">
 <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description rdf:about=""
   xmlns:xmp="http://ns.adobe.com/xap/1.0/"
   xmlns:dc="http://purl.org/dc/elements/1.1/"{attrs}>{body}
  </rdf:Description>
 </rdf:RDF>
</x:xmpmeta>
<?xpacket end="w"?>
"#,
        attrs = attrs,
        body = body
    )
}

/// Strip any namespace prefix: `dc:subject` and `subject` both arrive here.
fn local_name(raw: &[u8]) -> String {
    let name = String::from_utf8_lossy(raw);
    match name.rsplit_once(':') {
        Some((_, local)) => local.to_string(),
        None => name.to_string(),
    }
}

/// Resolve one entity reference to the text it stands for. Named entities
/// beyond the five XML predefines have no meaning in an XMP packet, so they
/// are dropped rather than guessed at.
fn resolve_entity(raw: &[u8]) -> Option<String> {
    match raw {
        b"amp" => Some("&".to_string()),
        b"lt" => Some("<".to_string()),
        b"gt" => Some(">".to_string()),
        b"quot" => Some("\"".to_string()),
        b"apos" => Some("\'".to_string()),
        _ => {
            // Numeric forms: &#38; and &#x26;
            let text = String::from_utf8_lossy(raw);
            let digits = text.strip_prefix('#')?;
            let code = match digits.strip_prefix('x').or_else(|| digits.strip_prefix('X')) {
                Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                None => digits.parse::<u32>().ok()?,
            };
            char::from_u32(code).map(|c| c.to_string())
        }
    }
}

/// Where the text currently being accumulated belongs.
#[derive(PartialEq, Eq, Clone, Copy)]
enum Target {
    None,
    Rating,
    Label,
    Tag,
    Description,
}

/// Read an XMP packet. Both spellings are accepted for every scalar field:
/// XMP allows `xmp:Rating="5"` as an attribute or `<xmp:Rating>5</xmp:Rating>`
/// as an element, and different tools emit different ones.
///
/// Text is accumulated across events rather than taken from the first one,
/// because the parser reports every entity reference separately: a note
/// reading `A & B` arrives as three events, not one.
pub fn parse(xml: &str) -> SidecarMetadata {
    let mut meta = SidecarMetadata::default();
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);

    let mut in_subject = false;
    let mut in_description = false;
    let mut target = Target::None;
    let mut buffer = String::new();

    let mut commit = |target: &mut Target, buffer: &mut String, meta: &mut SidecarMetadata| {
        let text = buffer.trim().to_string();
        buffer.clear();
        let current = std::mem::replace(target, Target::None);
        if text.is_empty() {
            return;
        }
        match current {
            Target::Rating => meta.rating = text.parse::<i32>().ok(),
            Target::Label => meta.label = Some(text),
            Target::Tag => {
                if !meta.tags.iter().any(|t| t == &text) {
                    meta.tags.push(text);
                }
            }
            Target::Description => {
                if meta.description.is_none() {
                    meta.description = Some(text);
                }
            }
            Target::None => {}
        }
    };

    loop {
        match reader.read_event() {
            Ok(Event::Eof) | Err(_) => break,
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref());

                for attribute in e.attributes().flatten() {
                    let key = local_name(attribute.key.as_ref());
                    let value = attribute
                        .unescape_value()
                        .map(|v| v.to_string())
                        .unwrap_or_default();
                    let value = value.trim();
                    match key.as_str() {
                        "Rating" => meta.rating = value.parse::<i32>().ok(),
                        "Label" if !value.is_empty() => meta.label = Some(value.to_string()),
                        _ => {}
                    }
                }

                buffer.clear();
                match name.as_str() {
                    "subject" => in_subject = true,
                    "description" => in_description = true,
                    "Rating" => target = Target::Rating,
                    "Label" => target = Target::Label,
                    "li" if in_subject => target = Target::Tag,
                    "li" if in_description => target = Target::Description,
                    _ => target = Target::None,
                }
            }
            Ok(Event::Text(e)) => {
                buffer.push_str(&String::from_utf8_lossy(e.as_ref()));
            }
            Ok(Event::GeneralRef(e)) => {
                if let Some(text) = resolve_entity(e.as_ref()) {
                    buffer.push_str(&text);
                }
            }
            Ok(Event::End(e)) => {
                let name = local_name(e.name().as_ref());
                commit(&mut target, &mut buffer, &mut meta);
                match name.as_str() {
                    "subject" => in_subject = false,
                    "description" => in_description = false,
                    _ => {}
                }
            }
            _ => {}
        }
    }

    meta
}

pub fn write_sidecar(file_path: &str, meta: &SidecarMetadata) -> Result<PathBuf, String> {
    let path = sidecar_path(file_path);
    std::fs::write(&path, build(meta))
        .map_err(|e| format!("Failed to write '{}': {}", path.display(), e))?;
    Ok(path)
}

/// `Ok(None)` when no sidecar exists, which is the ordinary case rather than
/// an error.
pub fn read_sidecar(file_path: &str) -> Result<Option<SidecarMetadata>, String> {
    let path = sidecar_path(file_path);
    if !path.is_file() {
        return Ok(None);
    }
    let xml = std::fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read '{}': {}", path.display(), e))?;
    Ok(Some(parse(&xml)))
}

// ---------------------------------------------------------------------------
// Batch export and import
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SidecarDirection {
    /// Database to sidecar files.
    Export,
    /// Sidecar files back into the database.
    Import,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SidecarProgress {
    pub processed: usize,
    pub total: usize,
    pub changed: usize,
    pub skipped: usize,
    pub failed: usize,
    pub cancelled: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SidecarResult {
    pub total: usize,
    pub processed: usize,
    pub changed: usize,
    pub skipped: usize,
    pub failed: usize,
    pub cancelled: bool,
    pub errors: Vec<String>,
}

#[derive(Default)]
pub struct SidecarState {
    pub cancelled: std::sync::atomic::AtomicBool,
    pub running: std::sync::atomic::AtomicBool,
}

#[derive(Default)]
pub struct SidecarCancellation(pub std::sync::Arc<SidecarState>);

const PROGRESS_INTERVAL_MS: u128 = 120;

pub fn begin(state: &SidecarState) -> Result<(), String> {
    use std::sync::atomic::Ordering;
    if state.running.swap(true, Ordering::SeqCst) {
        return Err("A metadata transfer is already running".to_string());
    }
    state.cancelled.store(false, Ordering::SeqCst);
    Ok(())
}

pub fn finish(state: &SidecarState) {
    state
        .running
        .store(false, std::sync::atomic::Ordering::SeqCst);
}

/// One file's worth of what the database knows, for export.
pub struct FileMetadata {
    pub file_id: i64,
    pub file_path: String,
    pub rating: i32,
    pub culling_flag: i32,
    pub is_favorite: bool,
    pub comments: Option<String>,
    pub tags: Vec<String>,
}

impl FileMetadata {
    fn to_sidecar(&self) -> SidecarMetadata {
        // A rejected photo is written as -1, which is how Bridge and Lightroom
        // both spell "rejected"; otherwise the star count carries across.
        let rating = if self.culling_flag == 2 {
            Some(-1)
        } else if self.rating > 0 {
            Some(self.rating)
        } else {
            None
        };
        SidecarMetadata {
            rating,
            label: self.is_favorite.then(|| FAVORITE_LABEL.to_string()),
            tags: self.tags.clone(),
            description: self
                .comments
                .as_ref()
                .map(|c| c.trim().to_string())
                .filter(|c| !c.is_empty()),
        }
    }
}

/// Write a sidecar for each file. Files with nothing worth recording are
/// counted as skipped rather than given an empty packet.
pub fn export_all<F, C>(
    files: Vec<FileMetadata>,
    mut report_progress: F,
    is_cancelled: C,
) -> SidecarResult
where
    F: FnMut(SidecarProgress),
    C: Fn() -> bool,
{
    let total = files.len();
    let mut changed = 0_usize;
    let mut skipped = 0_usize;
    let mut failed = 0_usize;
    let mut errors: Vec<String> = Vec::new();
    let mut last_emit = std::time::Instant::now();

    report_progress(SidecarProgress {
        processed: 0,
        total,
        changed: 0,
        skipped: 0,
        failed: 0,
        cancelled: false,
    });

    for (index, file) in files.into_iter().enumerate() {
        if is_cancelled() {
            return SidecarResult {
                total,
                processed: index,
                changed,
                skipped,
                failed,
                cancelled: true,
                errors,
            };
        }

        let meta = file.to_sidecar();
        if meta.is_empty() {
            skipped += 1;
        } else {
            match write_sidecar(&file.file_path, &meta) {
                Ok(_) => changed += 1,
                Err(e) => {
                    failed += 1;
                    if errors.len() < 10 {
                        errors.push(e);
                    }
                }
            }
        }

        let processed = index + 1;
        if processed == total || last_emit.elapsed().as_millis() >= PROGRESS_INTERVAL_MS {
            last_emit = std::time::Instant::now();
            report_progress(SidecarProgress {
                processed,
                total,
                changed,
                skipped,
                failed,
                cancelled: false,
            });
        }
    }

    SidecarResult {
        total,
        processed: total,
        changed,
        skipped,
        failed,
        cancelled: false,
        errors,
    }
}

/// What one file's sidecar says, ready to be applied to the database.
pub struct ImportedMetadata {
    pub file_id: i64,
    pub rating: Option<i32>,
    pub culling_reject: bool,
    pub is_favorite: Option<bool>,
    pub tags: Vec<String>,
    pub description: Option<String>,
}

/// Read the sidecar beside each file. Returns what was found; applying it to
/// the database is the caller's job, which keeps this free of query concerns.
pub fn import_all<F, C>(
    files: Vec<(i64, String)>,
    mut report_progress: F,
    is_cancelled: C,
) -> (Vec<ImportedMetadata>, SidecarResult)
where
    F: FnMut(SidecarProgress),
    C: Fn() -> bool,
{
    let total = files.len();
    let mut found = Vec::new();
    let mut skipped = 0_usize;
    let mut failed = 0_usize;
    let mut errors: Vec<String> = Vec::new();
    let mut last_emit = std::time::Instant::now();

    report_progress(SidecarProgress {
        processed: 0,
        total,
        changed: 0,
        skipped: 0,
        failed: 0,
        cancelled: false,
    });

    for (index, (file_id, file_path)) in files.into_iter().enumerate() {
        if is_cancelled() {
            let changed = found.len();
            return (
                found,
                SidecarResult {
                    total,
                    processed: index,
                    changed,
                    skipped,
                    failed,
                    cancelled: true,
                    errors,
                },
            );
        }

        match read_sidecar(&file_path) {
            Ok(Some(meta)) if !meta.is_empty() => {
                let is_favorite = meta
                    .label
                    .as_deref()
                    .map(|label| label.eq_ignore_ascii_case(FAVORITE_LABEL));
                found.push(ImportedMetadata {
                    file_id,
                    rating: meta.rating.filter(|r| *r > 0),
                    culling_reject: meta.rating == Some(-1),
                    is_favorite,
                    tags: meta.tags,
                    description: meta.description,
                });
            }
            Ok(_) => skipped += 1,
            Err(e) => {
                failed += 1;
                if errors.len() < 10 {
                    errors.push(e);
                }
            }
        }

        let processed = index + 1;
        if processed == total || last_emit.elapsed().as_millis() >= PROGRESS_INTERVAL_MS {
            last_emit = std::time::Instant::now();
            report_progress(SidecarProgress {
                processed,
                total,
                changed: found.len(),
                skipped,
                failed,
                cancelled: false,
            });
        }
    }

    let changed = found.len();
    (
        found,
        SidecarResult {
            total,
            processed: total,
            changed,
            skipped,
            failed,
            cancelled: false,
            errors,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sidecar_naming_follows_the_reading_tool() {
        // Raw files: Adobe replaces the extension.
        assert!(sidecar_path("D:/a/photo.cr2").ends_with("photo.xmp"));
        assert!(sidecar_path("D:/a/photo.NEF").ends_with("photo.xmp"));
        // Everything else appends, so siblings never collide.
        assert!(sidecar_path("D:/a/photo.jpg").ends_with("photo.jpg.xmp"));
        assert!(sidecar_path("D:/a/photo.png").ends_with("photo.png.xmp"));
        // A raw and a jpeg of the same shot get distinct sidecars.
        assert_ne!(sidecar_path("D:/a/x.cr2"), sidecar_path("D:/a/x.jpg"));
    }

    #[test]
    fn a_written_packet_reads_back_unchanged() {
        let meta = SidecarMetadata {
            rating: Some(4),
            label: Some(FAVORITE_LABEL.to_string()),
            tags: vec!["海边".to_string(), "family trip".to_string()],
            description: Some("夏天 & <the> beach".to_string()),
        };
        let parsed = parse(&build(&meta));
        assert_eq!(parsed, meta);
    }

    #[test]
    fn empty_metadata_round_trips_as_empty() {
        let meta = SidecarMetadata::default();
        assert!(meta.is_empty());
        let parsed = parse(&build(&meta));
        assert!(parsed.is_empty(), "{:?}", parsed);
    }

    #[test]
    fn a_rejected_photo_keeps_its_negative_rating() {
        let meta = SidecarMetadata {
            rating: Some(-1),
            ..Default::default()
        };
        assert_eq!(parse(&build(&meta)).rating, Some(-1));
    }

    #[test]
    fn scalars_written_as_elements_are_understood() {
        // Some tools spell these as child elements rather than attributes.
        let xml = r#"<?xpacket begin="" id="W5M0MpCehiHzreSzNTczkc9d"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/">
 <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description rdf:about=""
   xmlns:xmp="http://ns.adobe.com/xap/1.0/"
   xmlns:dc="http://purl.org/dc/elements/1.1/">
   <xmp:Rating>3</xmp:Rating>
   <xmp:Label>Red</xmp:Label>
   <dc:subject><rdf:Bag><rdf:li>sunset</rdf:li><rdf:li>beach</rdf:li></rdf:Bag></dc:subject>
   <dc:description><rdf:Alt><rdf:li xml:lang="x-default">a note</rdf:li></rdf:Alt></dc:description>
  </rdf:Description>
 </rdf:RDF>
</x:xmpmeta>
<?xpacket end="w"?>"#;
        let meta = parse(xml);
        assert_eq!(meta.rating, Some(3));
        assert_eq!(meta.label.as_deref(), Some("Red"));
        assert_eq!(meta.tags, vec!["sunset".to_string(), "beach".to_string()]);
        assert_eq!(meta.description.as_deref(), Some("a note"));
    }

    #[test]
    fn a_damaged_packet_yields_nothing_rather_than_failing() {
        let meta = parse("<x:xmpmeta><rdf:RDF><rdf:Description xmp:Rating=\"2\"");
        // Whatever survived is fine; the point is that it returns.
        assert!(meta.tags.is_empty());
    }
}
