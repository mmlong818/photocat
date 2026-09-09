//! Browsing a folder straight off disk, with nothing added to the library.
//!
//! The rest of the app addresses pictures by database row: thumbnails come
//! from `thumb://<library>/<file id>`, previews from `preview://…/<file id>`,
//! and the viewer starts from an id. That is the right design for a library
//! you keep, and the wrong one for "I just double-clicked a photo in a folder
//! I will never look at again" — there, being asked to import first is the
//! whole complaint.
//!
//! So this module addresses pictures by path instead. It lists a directory,
//! decodes any file the app can read, and caches thumbnails on disk keyed by
//! the file's own identity rather than by a row that does not exist. Nothing
//! here writes to the library database, and browsing leaves no trace in it.
//!
//! The cache is the one piece of state it does own, so it is bounded: entries
//! are pruned oldest-first once the directory passes [`CACHE_LIMIT_BYTES`].

use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::{Builder, Wry};

use crate::{t_image, t_utils};

/// Thumbnails are square-ish and small; this is the long edge in pixels.
const THUMBNAIL_SIZE: u32 = 256;

/// How large the on-disk thumbnail cache may grow before old entries go.
const CACHE_LIMIT_BYTES: u64 = 512 * 1024 * 1024;

/// One picture or video in the browsed folder.
#[derive(Debug, Clone, Serialize)]
pub struct BrowseFile {
    pub name: String,
    pub path: String,
    pub size: u64,
    /// Seconds since the epoch, or 0 when the filesystem will not say.
    pub modified: i64,
    /// Matches the rest of the app: 1 = image, 2 = video, 3 = RAW.
    pub file_type: i64,
    /// Identifies this exact revision of the file, for cache busting.
    pub signature: String,
}

/// A subdirectory, offered for navigation.
#[derive(Debug, Clone, Serialize)]
pub struct BrowseFolder {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct BrowseListing {
    pub folder: String,
    /// `None` at a drive root, where there is nowhere further up to go.
    pub parent: Option<String>,
    pub folders: Vec<BrowseFolder>,
    pub files: Vec<BrowseFile>,
}

/// How to order the files. Folders always sort by name, ahead of the files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortBy {
    Name,
    Date,
    Size,
    Type,
}

impl SortBy {
    pub fn from_i64(value: i64) -> Self {
        match value {
            1 => SortBy::Date,
            2 => SortBy::Size,
            3 => SortBy::Type,
            _ => SortBy::Name,
        }
    }
}

fn to_unix_seconds(time: std::io::Result<std::time::SystemTime>) -> i64 {
    time.ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// A short string that changes whenever the file's contents could have.
///
/// Size and modification time together are what every other thumbnailer uses
/// for this, and they are free — both come from the directory entry we already
/// read, with no second look at the file.
fn signature_from(size: u64, modified: i64) -> String {
    format!("{}-{}", modified, size)
}

/// Compare names the way a file manager does, so `img2` precedes `img10`.
///
/// Plain lexicographic order puts `img10` first, which reads as a bug to
/// anyone whose photos are numbered — which is to say, everyone's.
fn natural_key(name: &str) -> Vec<NaturalPart> {
    let mut parts = Vec::new();
    let mut digits = String::new();
    let mut text = String::new();

    for ch in name.chars() {
        if ch.is_ascii_digit() {
            if !text.is_empty() {
                parts.push(NaturalPart::Text(std::mem::take(&mut text)));
            }
            digits.push(ch);
        } else {
            if !digits.is_empty() {
                parts.push(NaturalPart::Number(
                    digits.parse().unwrap_or(u128::MAX),
                ));
                digits.clear();
            }
            text.push(ch.to_ascii_lowercase());
        }
    }
    if !digits.is_empty() {
        parts.push(NaturalPart::Number(digits.parse().unwrap_or(u128::MAX)));
    }
    if !text.is_empty() {
        parts.push(NaturalPart::Text(text));
    }
    parts
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
enum NaturalPart {
    /// Numbers sort before text at the same position, which keeps runs of
    /// numbered files together instead of interleaving them with prose names.
    Number(u128),
    Text(String),
}

fn extension_of(name: &str) -> String {
    Path::new(name)
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default()
}

/// List one directory: its subdirectories, and the media the app can display.
///
/// Not recursive. Browsing is a per-folder act, and walking a whole tree to
/// show one folder is the kind of surprise that makes an app feel slow.
pub fn list_folder(dir: &str, sort: SortBy, descending: bool) -> Result<BrowseListing, String> {
    let root = Path::new(dir);
    if !root.is_dir() {
        return Err(format!("not a folder: {}", dir));
    }

    let entries = fs::read_dir(root).map_err(|e| format!("cannot read {}: {}", dir, e))?;

    let mut folders = Vec::new();
    let mut files = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }

        let Ok(meta) = entry.metadata() else { continue };
        if meta.is_dir() {
            if is_hidden(&path) {
                continue;
            }
            folders.push(BrowseFolder {
                name,
                path: path.to_string_lossy().to_string(),
            });
            continue;
        }
        if !meta.is_file() {
            continue;
        }

        let path_str = path.to_string_lossy().to_string();
        let Some(file_type) = t_utils::get_file_type(&path_str) else {
            continue;
        };
        if !(1..=3).contains(&file_type) {
            continue;
        }

        let size = meta.len();
        let modified = to_unix_seconds(meta.modified());
        files.push(BrowseFile {
            name,
            path: path_str,
            size,
            modified,
            file_type,
            signature: signature_from(size, modified),
        });
    }

    folders.sort_by(|a, b| natural_key(&a.name).cmp(&natural_key(&b.name)));
    sort_files(&mut files, sort, descending);

    Ok(BrowseListing {
        folder: root.to_string_lossy().to_string(),
        parent: root.parent().map(|p| p.to_string_lossy().to_string()),
        folders,
        files,
    })
}

fn sort_files(files: &mut [BrowseFile], sort: SortBy, descending: bool) {
    files.sort_by(|a, b| {
        let ordering = match sort {
            SortBy::Name => natural_key(&a.name).cmp(&natural_key(&b.name)),
            SortBy::Date => a.modified.cmp(&b.modified),
            SortBy::Size => a.size.cmp(&b.size),
            // Within one extension, fall back to name so the order is stable
            // rather than whatever the filesystem happened to hand back.
            SortBy::Type => extension_of(&a.name)
                .cmp(&extension_of(&b.name))
                .then_with(|| natural_key(&a.name).cmp(&natural_key(&b.name))),
        };
        if descending { ordering.reverse() } else { ordering }
    });
}

#[cfg(target_os = "windows")]
fn is_hidden(path: &Path) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
    const FILE_ATTRIBUTE_SYSTEM: u32 = 0x4;
    fs::metadata(path)
        .map(|m| m.file_attributes() & (FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM) != 0)
        .unwrap_or(false)
}

#[cfg(not(target_os = "windows"))]
fn is_hidden(path: &Path) -> bool {
    path.file_name()
        .map(|n| n.to_string_lossy().starts_with('.'))
        .unwrap_or(false)
}

// ---------------------------------------------------------------- thumbnails

fn cache_dir() -> Result<PathBuf, String> {
    let dir = crate::t_config::get_app_data_dir()?.join("browse-thumbs");
    fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {}", dir.display(), e))?;
    Ok(dir)
}

/// A file name for the cached thumbnail of this exact revision of this file.
///
/// The path is hashed rather than sanitised: paths are long, contain
/// separators and any character the filesystem allows, and only the mapping
/// needs to be stable, not readable.
fn cache_name(path: &str, signature: &str) -> String {
    let mut hasher = DefaultHasher::new();
    path.hash(&mut hasher);
    format!("{:016x}-{}.jpg", hasher.finish(), signature)
}

/// The thumbnail for one file, generating and caching it on first request.
pub fn thumbnail(path: &str, signature: &str) -> Result<Vec<u8>, String> {
    let dir = cache_dir()?;
    let cached = dir.join(cache_name(path, signature));
    if let Ok(data) = fs::read(&cached) {
        if !data.is_empty() {
            return Ok(data);
        }
    }

    let file_type = t_utils::get_file_type(path).unwrap_or(0);
    let orientation = t_image::get_image_orientation(path);
    let generated = match file_type {
        3 => t_image::get_raw_thumbnail(path, orientation, THUMBNAIL_SIZE, false),
        2 => crate::t_video::get_video_thumbnail_sync(path, THUMBNAIL_SIZE, None, None),
        _ => t_image::get_image_thumbnail(path, orientation, THUMBNAIL_SIZE),
    };

    let data = generated
        .map_err(|e| format!("cannot make a thumbnail for {}: {}", path, e))?
        .ok_or_else(|| format!("no thumbnail for {}", path))?;

    // A failed write costs a regenerated thumbnail next time, nothing worse,
    // so it is not worth failing the request over.
    let _ = fs::write(&cached, &data);
    Ok(data)
}

/// Drop the oldest cached thumbnails until the cache fits in `limit` bytes.
///
/// Called once at startup, off the main thread. Browsing many large folders
/// would otherwise grow this without bound, and it is pure cache: anything
/// deleted is regenerated on demand.
pub fn prune_cache(limit: u64) -> Result<u64, String> {
    let dir = cache_dir()?;
    let mut entries: Vec<(PathBuf, i64, u64)> = fs::read_dir(&dir)
        .map_err(|e| e.to_string())?
        .flatten()
        .filter_map(|entry| {
            let meta = entry.metadata().ok()?;
            if !meta.is_file() {
                return None;
            }
            Some((entry.path(), to_unix_seconds(meta.modified()), meta.len()))
        })
        .collect();

    let mut total: u64 = entries.iter().map(|(_, _, size)| size).sum();
    if total <= limit {
        return Ok(0);
    }

    entries.sort_by_key(|(_, modified, _)| *modified);
    let mut freed = 0u64;
    for (path, _, size) in entries {
        if total <= limit {
            break;
        }
        if fs::remove_file(&path).is_ok() {
            total = total.saturating_sub(size);
            freed += size;
        }
    }
    Ok(freed)
}

/// Prune in the background at startup, reporting only if something went wrong.
pub fn prune_cache_in_background() {
    std::thread::spawn(|| match prune_cache(CACHE_LIMIT_BYTES) {
        Ok(0) => {}
        Ok(freed) => println!("browse cache: freed {} MB", freed / 1048576),
        Err(e) => eprintln!("browse cache: {}", e),
    });
}

// ------------------------------------------------------------------ protocol

fn query_value(uri: &http::Uri, key: &str) -> Option<String> {
    let query = uri.query()?;
    for pair in query.split('&') {
        let mut parts = pair.splitn(2, '=');
        if parts.next()? == key {
            let raw = parts.next().unwrap_or("");
            return Some(percent_decode(raw));
        }
    }
    None
}

/// Decode the percent-escapes a URL query needs, including `+` for a space.
fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'%' if index + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[index + 1..index + 3]).unwrap_or("");
                match u8::from_str_radix(hex, 16) {
                    Ok(byte) => {
                        out.push(byte);
                        index += 3;
                    }
                    Err(_) => {
                        out.push(bytes[index]);
                        index += 1;
                    }
                }
            }
            b'+' => {
                out.push(b' ');
                index += 1;
            }
            byte => {
                out.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).to_string()
}

/// Serve browsed files by path: `browse://localhost/thumb?path=…&v=…`
/// for the grid, and `/full?path=…&v=…` for anything the webview cannot
/// decode itself (RAW, HEIC, TIFF and friends).
pub fn register_protocol(builder: Builder<Wry>) -> Builder<Wry> {
    builder.register_asynchronous_uri_scheme_protocol("browse", |_ctx, request, responder| {
        let uri = request.uri().clone();
        let kind = uri.path().trim_start_matches('/').to_string();
        let Some(path) = query_value(&uri, "path") else {
            responder.respond(crate::t_protocol::text_response(
                http::StatusCode::BAD_REQUEST,
                "missing path",
            ));
            return;
        };
        let signature = query_value(&uri, "v").unwrap_or_default();

        tauri::async_runtime::spawn(async move {
            let response = match kind.as_str() {
                "thumb" => match tauri::async_runtime::spawn_blocking(move || {
                    thumbnail(&path, &signature)
                })
                .await
                {
                    Ok(Ok(data)) => crate::t_protocol::image_response(data),
                    _ => crate::t_protocol::text_response(
                        http::StatusCode::NOT_FOUND,
                        "thumbnail not available",
                    ),
                },
                "full" => match t_image::get_file_image_bytes_cached(&path, false).await {
                    Ok(data) => crate::t_protocol::image_response(data),
                    Err(_) => crate::t_protocol::text_response(
                        http::StatusCode::NOT_FOUND,
                        "image not available",
                    ),
                },
                _ => crate::t_protocol::text_response(
                    http::StatusCode::NOT_FOUND,
                    "unknown browse request",
                ),
            };
            responder.respond(response);
        });
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(name: &str, modified: i64, size: u64) -> BrowseFile {
        BrowseFile {
            name: name.to_string(),
            path: format!("D:/x/{}", name),
            size,
            modified,
            file_type: 1,
            signature: signature_from(size, modified),
        }
    }

    fn names(files: &[BrowseFile]) -> Vec<&str> {
        files.iter().map(|f| f.name.as_str()).collect()
    }

    #[test]
    fn numbers_in_names_sort_by_value_not_by_digit() {
        let mut files = vec![file("img10.jpg", 0, 0), file("img2.jpg", 0, 0), file("img1.jpg", 0, 0)];
        sort_files(&mut files, SortBy::Name, false);
        assert_eq!(names(&files), ["img1.jpg", "img2.jpg", "img10.jpg"]);
    }

    #[test]
    fn name_sorting_ignores_case() {
        let mut files = vec![file("Beta.jpg", 0, 0), file("alpha.jpg", 0, 0)];
        sort_files(&mut files, SortBy::Name, false);
        assert_eq!(names(&files), ["alpha.jpg", "Beta.jpg"]);
    }

    #[test]
    fn descending_reverses_every_order() {
        let mut files = vec![file("a.jpg", 10, 1), file("b.jpg", 20, 2)];
        sort_files(&mut files, SortBy::Date, true);
        assert_eq!(names(&files), ["b.jpg", "a.jpg"]);
        sort_files(&mut files, SortBy::Size, true);
        assert_eq!(names(&files), ["b.jpg", "a.jpg"]);
    }

    #[test]
    fn same_extension_falls_back_to_name() {
        let mut files = vec![
            file("b.jpg", 0, 0),
            file("a.png", 0, 0),
            file("a.jpg", 0, 0),
        ];
        sort_files(&mut files, SortBy::Type, false);
        assert_eq!(names(&files), ["a.jpg", "b.jpg", "a.png"]);
    }

    #[test]
    fn a_changed_file_gets_a_different_cache_entry() {
        let first = cache_name("D:/photos/a.jpg", &signature_from(100, 5));
        let touched = cache_name("D:/photos/a.jpg", &signature_from(100, 6));
        let other = cache_name("D:/photos/b.jpg", &signature_from(100, 5));
        assert_ne!(first, touched, "a rewritten file must not reuse the thumbnail");
        assert_ne!(first, other);
        assert_eq!(first, cache_name("D:/photos/a.jpg", &signature_from(100, 5)));
    }

    #[test]
    fn query_values_are_percent_decoded() {
        let uri: http::Uri = "browse://localhost/thumb?path=D%3A%2F%E7%85%A7%E7%89%87%2Fa%20b.jpg&v=1-2"
            .parse()
            .unwrap();
        assert_eq!(query_value(&uri, "path").unwrap(), "D:/照片/a b.jpg");
        assert_eq!(query_value(&uri, "v").unwrap(), "1-2");
        assert_eq!(query_value(&uri, "missing"), None);
    }

    #[test]
    fn a_plus_in_a_query_is_a_space() {
        assert_eq!(percent_decode("a+b%2Bc"), "a b+c");
    }

    #[test]
    fn listing_a_missing_folder_is_an_error_not_an_empty_list() {
        assert!(list_folder("D:/definitely-not-here-9182", SortBy::Name, false).is_err());
    }

    #[test]
    fn a_real_folder_lists_only_media() {
        let dir = std::env::temp_dir().join("lap-browse-test");
        let _ = fs::create_dir_all(&dir);
        fs::write(dir.join("note.txt"), b"not a picture").unwrap();
        fs::write(dir.join("a.jpg"), b"pretend jpeg").unwrap();
        let _ = fs::create_dir_all(dir.join("sub"));

        let listing = list_folder(&dir.to_string_lossy(), SortBy::Name, false).unwrap();
        assert_eq!(names(&listing.files), ["a.jpg"], "text files are not media");
        assert!(listing.folders.iter().any(|f| f.name == "sub"));
        assert!(listing.parent.is_some());

        let _ = fs::remove_dir_all(&dir);
    }
}
