/**
 * Batch renaming.
 *
 * Files arrive named by whatever produced them — `IMG_0423.JPG`,
 * `DSC_9981.NEF`, `ScreenShot_2026-01-22_015447_607.png` — and stay that way,
 * because renaming a few hundred of them by hand is not worth the afternoon.
 * A template turns that into one operation: choose a shape once, see it
 * applied to the whole selection before committing, and rename.
 *
 * The renaming itself goes through the app's existing single-file rename, so
 * Live Photo components, `.aae` sidecars and RAW+JPEG pairs keep moving
 * together and the database stays consistent. This module only decides what
 * each file should be called.
 */
use chrono::{Local, TimeZone};

/// Everything a template can refer to for one file.
pub struct RenameSource {
    pub file_id: i64,
    pub file_path: String,
    /// Current file name including its extension.
    pub name: String,
    /// Capture time in milliseconds, when the file has one.
    pub taken_date: Option<i64>,
    pub make: Option<String>,
    pub model: Option<String>,
    pub lens: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RenamePreview {
    pub file_id: i64,
    pub old_name: String,
    pub new_name: String,
    /// Set when this file cannot be renamed, and why.
    pub error: Option<String>,
}

/// Characters Windows forbids outright, plus the separators, replaced rather
/// than dropped so a template mistake produces a readable name.
fn sanitize(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if (c as u32) < 0x20 => '_',
            c => c,
        })
        .collect::<String>()
        .trim()
        .trim_end_matches('.')
        .to_string()
}

fn stem_and_extension(name: &str) -> (&str, &str) {
    match name.rfind('.') {
        // A leading dot is part of the name, not an extension.
        Some(index) if index > 0 => (&name[..index], &name[index + 1..]),
        _ => (name, ""),
    }
}

/// Expand one `{token}` against a file. Unknown tokens are left as they were
/// typed, so a mistake is visible in the preview rather than silently dropped.
fn expand_token(token: &str, source: &RenameSource, index: usize) -> String {
    let (name, argument) = match token.split_once(':') {
        Some((name, argument)) => (name, Some(argument)),
        None => (token, None),
    };

    let taken = source
        .taken_date
        .and_then(|ms| Local.timestamp_millis_opt(ms).single());

    match name {
        "name" => stem_and_extension(&source.name).0.to_string(),
        "n" => {
            let width = argument.and_then(|a| a.parse::<usize>().ok()).unwrap_or(1);
            format!("{:0width$}", index, width = width.clamp(1, 9))
        }
        "date" => taken
            .map(|t| t.format("%Y-%m-%d").to_string())
            .unwrap_or_default(),
        "time" => taken
            .map(|t| t.format("%H%M%S").to_string())
            .unwrap_or_default(),
        "year" => taken.map(|t| t.format("%Y").to_string()).unwrap_or_default(),
        "month" => taken.map(|t| t.format("%m").to_string()).unwrap_or_default(),
        "day" => taken.map(|t| t.format("%d").to_string()).unwrap_or_default(),
        "make" => source.make.clone().unwrap_or_default(),
        "camera" => source.model.clone().unwrap_or_default(),
        "lens" => source.lens.clone().unwrap_or_default(),
        _ => format!("{{{}}}", token),
    }
}

/// Apply `template` to one file, producing the name without its extension.
/// Kept separate so callers can tell an empty result from a name that merely
/// starts with a dot.
pub fn render_stem(template: &str, source: &RenameSource, index: usize) -> String {
    let mut out = String::with_capacity(template.len() + 16);
    let mut rest = template;

    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        match rest[start + 1..].find('}') {
            Some(offset) => {
                let token = &rest[start + 1..start + 1 + offset];
                out.push_str(&expand_token(token, source, index));
                rest = &rest[start + 1 + offset + 1..];
            }
            None => {
                // An unclosed brace is literal text.
                out.push_str(&rest[start..]);
                rest = "";
                break;
            }
        }
    }
    out.push_str(rest);
    sanitize(&out)
}

/// The full new file name. The extension is never part of the template and is
/// always carried over unchanged.
pub fn render(template: &str, source: &RenameSource, index: usize) -> String {
    let stem = render_stem(template, source, index);
    let extension = stem_and_extension(&source.name).1;
    if extension.is_empty() {
        stem
    } else {
        format!("{}.{}", stem, extension)
    }
}

/// Add `(1)`, `(2)` … until the name is free. `taken` holds names already
/// claimed earlier in this batch, which is what stops two files from being
/// given the same name in one pass.
fn make_unique(
    folder: &std::path::Path,
    candidate: &str,
    taken: &std::collections::HashSet<String>,
) -> String {
    let is_free = |name: &str| {
        !taken.contains(&name.to_lowercase()) && !folder.join(name).exists()
    };
    if is_free(candidate) {
        return candidate.to_string();
    }
    let (stem, extension) = stem_and_extension(candidate);
    for counter in 1..10_000 {
        let next = if extension.is_empty() {
            format!("{}({})", stem, counter)
        } else {
            format!("{}({}).{}", stem, counter, extension)
        };
        if is_free(&next) {
            return next;
        }
    }
    candidate.to_string()
}

/// Work out what every file in the batch would be called, in the order given.
/// The same function drives both the preview and the rename, so what the user
/// approves is exactly what happens.
pub fn plan(sources: &[RenameSource], template: &str, start_index: usize) -> Vec<RenamePreview> {
    let mut claimed: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut out = Vec::with_capacity(sources.len());

    for (offset, source) in sources.iter().enumerate() {
        let stem = render_stem(template, source, start_index + offset);
        if stem.trim().is_empty() {
            out.push(RenamePreview {
                file_id: source.file_id,
                old_name: source.name.clone(),
                new_name: source.name.clone(),
                error: Some("The template produces an empty name".to_string()),
            });
            continue;
        }

        let rendered = render(template, source, start_index + offset);
        let folder = std::path::Path::new(&source.file_path)
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_default();

        // A file keeping its own name is not a conflict with itself.
        let unchanged = rendered.eq_ignore_ascii_case(&source.name);
        let final_name = if unchanged {
            rendered
        } else {
            make_unique(&folder, &rendered, &claimed)
        };
        claimed.insert(final_name.to_lowercase());

        out.push(RenamePreview {
            file_id: source.file_id,
            old_name: source.name.clone(),
            new_name: final_name,
            error: None,
        });
    }

    out
}

// ---------------------------------------------------------------------------
// Applying a plan
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameProgress {
    pub processed: usize,
    pub total: usize,
    pub renamed: usize,
    pub skipped: usize,
    pub failed: usize,
    pub cancelled: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameResult {
    pub total: usize,
    pub processed: usize,
    pub renamed: usize,
    pub skipped: usize,
    pub failed: usize,
    pub cancelled: bool,
    pub errors: Vec<String>,
}

#[derive(Default)]
pub struct RenameState {
    pub cancelled: std::sync::atomic::AtomicBool,
    pub running: std::sync::atomic::AtomicBool,
}

#[derive(Default)]
pub struct RenameCancellation(pub std::sync::Arc<RenameState>);

const PROGRESS_INTERVAL_MS: u128 = 120;

pub fn begin(state: &RenameState) -> Result<(), String> {
    use std::sync::atomic::Ordering;
    if state.running.swap(true, Ordering::SeqCst) {
        return Err("A rename is already running".to_string());
    }
    state.cancelled.store(false, Ordering::SeqCst);
    Ok(())
}

pub fn finish(state: &RenameState) {
    state
        .running
        .store(false, std::sync::atomic::Ordering::SeqCst);
}

/// Carry out a plan. `rename_one` performs the actual rename, which lets the
/// caller supply the app's existing single-file path so grouped assets and the
/// database stay in step.
pub fn apply<R, F, C>(
    sources: &[RenameSource],
    plan: Vec<RenamePreview>,
    mut rename_one: R,
    mut report_progress: F,
    is_cancelled: C,
) -> RenameResult
where
    R: FnMut(i64, &str, &str) -> Result<(), String>,
    F: FnMut(RenameProgress),
    C: Fn() -> bool,
{
    let total = plan.len();
    let mut renamed = 0_usize;
    let mut skipped = 0_usize;
    let mut failed = 0_usize;
    let mut errors: Vec<String> = Vec::new();
    let mut last_emit = std::time::Instant::now();

    report_progress(RenameProgress {
        processed: 0,
        total,
        renamed: 0,
        skipped: 0,
        failed: 0,
        cancelled: false,
    });

    for (index, entry) in plan.into_iter().enumerate() {
        if is_cancelled() {
            return RenameResult {
                total,
                processed: index,
                renamed,
                skipped,
                failed,
                cancelled: true,
                errors,
            };
        }

        let source = &sources[index];
        if entry.error.is_some() || entry.new_name == entry.old_name {
            skipped += 1;
        } else {
            match rename_one(entry.file_id, &source.file_path, &entry.new_name) {
                Ok(()) => renamed += 1,
                Err(e) => {
                    failed += 1;
                    if errors.len() < 10 {
                        errors.push(format!("{} → {}: {}", entry.old_name, entry.new_name, e));
                    }
                }
            }
        }

        let processed = index + 1;
        if processed == total || last_emit.elapsed().as_millis() >= PROGRESS_INTERVAL_MS {
            last_emit = std::time::Instant::now();
            report_progress(RenameProgress {
                processed,
                total,
                renamed,
                skipped,
                failed,
                cancelled: false,
            });
        }
    }

    RenameResult {
        total,
        processed: total,
        renamed,
        skipped,
        failed,
        cancelled: false,
        errors,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(name: &str) -> RenameSource {
        RenameSource {
            file_id: 1,
            file_path: format!("D:/photos/{}", name),
            name: name.to_string(),
            // 2024-05-17 14:23:45 local time.
            taken_date: Local
                .with_ymd_and_hms(2024, 5, 17, 14, 23, 45)
                .single()
                .map(|t| t.timestamp_millis()),
            make: Some("Canon".to_string()),
            model: Some("EOS R6".to_string()),
            lens: Some("RF 24-70mm".to_string()),
        }
    }

    #[test]
    fn the_extension_is_always_carried_over() {
        assert_eq!(render("holiday", &source("IMG_1.JPG"), 1), "holiday.JPG");
        // Even when the template mentions one, it is treated as part of the stem.
        assert_eq!(render("a.b", &source("IMG_1.JPG"), 1), "a.b.JPG");
        // A file with no extension stays without one.
        assert_eq!(render("x", &source("README"), 1), "x");
    }

    #[test]
    fn tokens_expand_from_the_file() {
        let s = source("IMG_0423.JPG");
        assert_eq!(render("{name}", &s, 1), "IMG_0423.JPG");
        assert_eq!(render("{date}", &s, 1), "2024-05-17.JPG");
        assert_eq!(render("{year}{month}{day}", &s, 1), "20240517.JPG");
        assert_eq!(render("{time}", &s, 1), "142345.JPG");
        assert_eq!(render("{camera}", &s, 1), "EOS R6.JPG");
        assert_eq!(render("{make} {camera}", &s, 1), "Canon EOS R6.JPG");
        assert_eq!(render("{lens}", &s, 1), "RF 24-70mm.JPG");
    }

    #[test]
    fn the_sequence_number_pads_to_the_requested_width() {
        let s = source("a.jpg");
        assert_eq!(render("{n}", &s, 7), "7.jpg");
        assert_eq!(render("{n:3}", &s, 7), "007.jpg");
        assert_eq!(render("{n:3}", &s, 1234), "1234.jpg");
        assert_eq!(render("shot-{n:2}", &s, 5), "shot-05.jpg");
    }

    #[test]
    fn characters_illegal_in_a_file_name_are_replaced() {
        let s = source("a.jpg");
        assert_eq!(render("a/b:c*d?", &s, 1), "a_b_c_d_.jpg");
        // A trailing dot or space would be silently dropped by Windows.
        assert_eq!(render("name.", &s, 1), "name.jpg");
        assert_eq!(render("  spaced  ", &s, 1), "spaced.jpg");
    }

    #[test]
    fn an_unknown_or_unclosed_token_stays_visible() {
        let s = source("a.jpg");
        assert_eq!(render("{nope}", &s, 1), "{nope}.jpg");
        assert_eq!(render("{unclosed", &s, 1), "{unclosed.jpg");
        assert_eq!(render("plain", &s, 1), "plain.jpg");
    }

    #[test]
    fn a_file_without_a_capture_time_leaves_date_tokens_empty() {
        let mut s = source("a.jpg");
        s.taken_date = None;
        assert_eq!(render("x{date}y", &s, 1), "xy.jpg");
    }

    #[test]
    fn two_files_never_receive_the_same_name() {
        let sources = vec![
            RenameSource { file_id: 1, name: "a.jpg".into(), ..source("a.jpg") },
            RenameSource { file_id: 2, name: "b.jpg".into(), ..source("b.jpg") },
            RenameSource { file_id: 3, name: "c.jpg".into(), ..source("c.jpg") },
        ];
        // A template with no varying part would otherwise collide.
        let plan = plan(&sources, "trip", 1);
        assert_eq!(plan[0].new_name, "trip.jpg");
        assert_eq!(plan[1].new_name, "trip(1).jpg");
        assert_eq!(plan[2].new_name, "trip(2).jpg");
        assert!(plan.iter().all(|p| p.error.is_none()));
    }

    #[test]
    fn the_sequence_advances_across_the_batch() {
        let sources = vec![source("a.jpg"), source("b.png"), source("c.jpg")];
        let plan = plan(&sources, "{n:3}", 10);
        assert_eq!(plan[0].new_name, "010.jpg");
        assert_eq!(plan[1].new_name, "011.png");
        assert_eq!(plan[2].new_name, "012.jpg");
    }

    #[test]
    fn an_empty_result_is_reported_rather_than_applied() {
        let sources = vec![source("a.jpg")];
        let plan = plan(&sources, "", 1);
        assert_eq!(plan[0].new_name, "a.jpg");
        assert!(plan[0].error.is_some());
    }
}
