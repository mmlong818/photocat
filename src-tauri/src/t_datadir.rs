/**
 * One-time migration of app data from a previously used bundle identifier.
 *
 * The per-user data directory is derived from the bundle identifier
 * (`%LOCALAPPDATA%\<identifier>` on Windows, `~/Library/Application Support`
 * and `~/Library/Caches` on macOS, XDG dirs on Linux). Renaming the identifier
 * therefore orphans every existing library, so on first run under the new
 * identifier we carry the old data across.
 *
 * What moves:
 *   - `app-config.json`      the library list and app settings
 *   - `libraries/`           the SQLite databases (real copies)
 *   - `index-recovery-*.json` interrupted-scan recovery state
 *   - `<library id>/`        thumbnail caches, hard-linked when the filesystem
 *                            allows it so the copy is instant and free
 *
 * What does not move: `EBWebView` (webview cache) and `video_cache`, both of
 * which regenerate on demand.
 *
 * The old directory is left untouched, so a failed migration is recoverable by
 * pointing the app back at the old identifier.
 */
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

use crate::t_config;

/// The identifier this fork shipped under before it was renamed.
pub const LEGACY_IDENTIFIER: &str = "com.julyx10.lap";

/// Directories under the data root that are caches we deliberately drop.
const SKIP_DIRS: &[&str] = &["EBWebView", "video_cache", "libraries"];

#[derive(Debug, Default)]
pub struct MigrationReport {
    pub libraries: usize,
    pub db_files: u64,
    pub thumbs_linked: u64,
    pub thumbs_copied: u64,
}

impl MigrationReport {
    pub fn thumbs_total(&self) -> u64 {
        self.thumbs_linked + self.thumbs_copied
    }
}

/// Mirror of `t_config::get_app_data_folder_name` for an arbitrary identifier.
fn folder_name(identifier: &str) -> String {
    if cfg!(debug_assertions) {
        format!("{}.debug", identifier)
    } else {
        identifier.to_string()
    }
}

fn legacy_data_dir(identifier: &str) -> Option<PathBuf> {
    dirs::data_local_dir().map(|p| p.join(folder_name(identifier)))
}

fn legacy_cache_dir(identifier: &str) -> Option<PathBuf> {
    dirs::cache_dir().map(|p| p.join(folder_name(identifier)))
}

/// Copy one file, replacing any existing target.
fn copy_file(src: &Path, dst: &Path) -> Result<(), String> {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create '{}': {}", parent.display(), e))?;
    }
    fs::copy(src, dst)
        .map(|_| ())
        .map_err(|e| format!("Failed to copy '{}': {}", src.display(), e))
}

/// Hard-link a file when the filesystem supports it, otherwise copy it.
/// Returns true when the file was linked rather than copied.
fn link_or_copy(src: &Path, dst: &Path) -> Result<bool, String> {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create '{}': {}", parent.display(), e))?;
    }
    if dst.exists() {
        return Ok(false);
    }
    match fs::hard_link(src, dst) {
        Ok(()) => Ok(true),
        Err(_) => {
            // Cross-volume, unsupported filesystem, or a link-count limit.
            fs::copy(src, dst)
                .map(|_| false)
                .map_err(|e| format!("Failed to copy '{}': {}", src.display(), e))
        }
    }
}

/// Recursively bring a thumbnail cache tree across, preferring hard links.
fn link_tree(src: &Path, dst: &Path, report: &mut MigrationReport) {
    let entries = match fs::read_dir(src) {
        Ok(entries) => entries,
        Err(e) => {
            eprintln!("migrate: cannot read '{}': {}", src.display(), e);
            return;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let target = dst.join(entry.file_name());
        if path.is_dir() {
            link_tree(&path, &target, report);
        } else {
            match link_or_copy(&path, &target) {
                Ok(true) => report.thumbs_linked += 1,
                Ok(false) => report.thumbs_copied += 1,
                Err(e) => eprintln!("migrate: {}", e),
            }
        }
    }
}

/// Library ids named by the migrated config, so we only touch cache
/// directories we can actually account for.
fn library_ids_from_config(config_path: &Path) -> Vec<String> {
    let Ok(text) = fs::read_to_string(config_path) else {
        return Vec::new();
    };
    let Ok(value) = serde_json::from_str::<Value>(&text) else {
        return Vec::new();
    };
    value
        .get("libraries")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("id").and_then(Value::as_str))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// Run the migration if, and only if, this identifier has never been used
/// before and the legacy one has data. Returns `None` when nothing was done.
pub fn migrate_from_legacy(current_identifier: &str) -> Option<MigrationReport> {
    if current_identifier == LEGACY_IDENTIFIER {
        return None;
    }

    let new_data = t_config::get_app_data_dir().ok()?;
    let new_config = new_data.join("app-config.json");
    if new_config.exists() {
        // This identifier has already been used; never overwrite live data.
        return None;
    }

    let old_data = legacy_data_dir(LEGACY_IDENTIFIER)?;
    let old_config = old_data.join("app-config.json");
    if !old_config.exists() {
        return None;
    }

    eprintln!(
        "migrate: importing data from '{}' into '{}'",
        old_data.display(),
        new_data.display()
    );

    let mut report = MigrationReport::default();

    if let Err(e) = fs::create_dir_all(&new_data) {
        eprintln!("migrate: cannot create '{}': {}", new_data.display(), e);
        return None;
    }

    // 1. Library databases first: without them the config points at nothing.
    let old_libraries = old_data.join("libraries");
    let new_libraries = new_data.join("libraries");
    if old_libraries.is_dir() {
        if let Ok(entries) = fs::read_dir(&old_libraries) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                match copy_file(&path, &new_libraries.join(entry.file_name())) {
                    Ok(()) => report.db_files += 1,
                    Err(e) => {
                        eprintln!("migrate: {}", e);
                        // A partial library set is worse than none at all.
                        let _ = fs::remove_dir_all(&new_libraries);
                        return None;
                    }
                }
            }
        }
    }

    // 2. Interrupted-scan recovery state.
    if let Ok(entries) = fs::read_dir(&old_data) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with("index-recovery-") && name.ends_with(".json") {
                if let Err(e) = copy_file(&entry.path(), &new_data.join(entry.file_name())) {
                    eprintln!("migrate: {}", e);
                }
            }
        }
    }

    // 3. The config last, so its presence marks the migration as complete.
    if let Err(e) = copy_file(&old_config, &new_config) {
        eprintln!("migrate: {}", e);
        let _ = fs::remove_dir_all(&new_libraries);
        return None;
    }

    // 4. Thumbnail caches, one directory per library id.
    let ids = library_ids_from_config(&new_config);
    report.libraries = ids.len();
    if let (Some(old_cache), Ok(new_cache)) = (
        legacy_cache_dir(LEGACY_IDENTIFIER),
        t_config::get_app_cache_dir(),
    ) {
        for id in &ids {
            if SKIP_DIRS.contains(&id.as_str()) {
                continue;
            }
            let src = old_cache.join(id);
            if src.is_dir() {
                link_tree(&src, &new_cache.join(id), &mut report);
            }
        }
    }

    eprintln!(
        "migrate: done — {} librar{}, {} db file(s), {} thumbnail(s) ({} linked, {} copied)",
        report.libraries,
        if report.libraries == 1 { "y" } else { "ies" },
        report.db_files,
        report.thumbs_total(),
        report.thumbs_linked,
        report.thumbs_copied
    );

    Some(report)
}

/// Holds the outcome of a first-run migration until the frontend collects it.
/// The migration finishes long before any window exists, so an event emitted
/// at that point would be lost.
#[derive(Default)]
pub struct MigrationOutcome(std::sync::Mutex<Option<MigrationReport>>);

impl MigrationOutcome {
    pub fn set(&self, report: MigrationReport) {
        if let Ok(mut slot) = self.0.lock() {
            *slot = Some(report);
        }
    }
}

#[derive(serde::Serialize)]
pub struct MigrationSummary {
    pub libraries: usize,
    pub thumbnails: u64,
}

/// Returns the migration summary exactly once, then forgets it.
#[tauri::command]
pub fn take_migration_report(
    state: tauri::State<MigrationOutcome>,
) -> Option<MigrationSummary> {
    let report = state.0.lock().ok()?.take()?;
    Some(MigrationSummary {
        libraries: report.libraries,
        thumbnails: report.thumbs_total(),
    })
}
