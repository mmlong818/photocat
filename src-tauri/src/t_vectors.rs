/**
 * In-memory embedding cache for semantic image search.
 *
 * Semantic search stores one CLIP embedding per image in `afiles.embeds`.
 * Answering a query used to mean asking SQLite for every one of those BLOBs,
 * allocating a `Vec<u8>` per row and decoding it — roughly 2 KB of database
 * reads and one allocation per indexed image, on every search. At 100k images
 * that is ~200 MB of I/O per query.
 *
 * The cost was never the arithmetic; it was the I/O. So this module keeps the
 * vectors resident in one contiguous `f32` slab with their norms precomputed,
 * and scores against that instead. The comparison itself stays exhaustive, so
 * results are **identical** to the database scan, down to the ranking.
 *
 * An approximate index (HNSW, already vendored for similar-photo grouping) was
 * measured first and rejected: semantic search promises *every* file above the
 * threshold, and approximate search silently dropped true matches — a
 * different subset on each run, because the graph is built in parallel. A
 * fluctuating result set is unacceptable for smart albums, whose counts are
 * shown in the UI and whose ids seed the dedup and similar-photo scopes.
 *
 * The slab is built lazily on first search, revalidated on every query against
 * a cheap `(count, max id)` fingerprint so newly embedded or deleted files
 * cannot return stale hits, and dropped when the active library changes.
 *
 * Set `LAP_VECTOR_CACHE=0` to bypass the cache and read from SQLite as before.
 */
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};

use crate::t_sqlite;
use crate::t_storage;

/// Below this many embedded images a database scan is already imperceptible,
/// and holding a second copy of the vectors in memory is not worth it.
const MIN_CACHED: usize = 1_000;

/// Refuse to cache beyond this many vectors. At 512 dimensions this caps the
/// slab at about 1 GB, past which the database scan is the safer behaviour.
const MAX_CACHED: usize = 500_000;

/// All embeddings for one library, laid out for a fast linear scan.
struct VectorCache {
    /// Absolute path of the library database these vectors came from.
    library_key: String,
    /// Embedding width. A query of any other width cannot use this cache.
    dim: usize,
    /// `(rows with an embedding, largest file id with an embedding)`.
    fingerprint: (i64, i64),
    /// `afiles.id` for row `n`.
    ids: Vec<i64>,
    /// Row `n` occupies `data[n * dim .. (n + 1) * dim]`.
    data: Vec<f32>,
    /// L2 norm of row `n`, precomputed once instead of per query.
    norms: Vec<f32>,
}

impl VectorCache {
    /// Every `(file id, cosine similarity)` scoring strictly above
    /// `threshold`, best first. Exhaustive, so the result matches the
    /// database scan exactly.
    fn search(&self, query: &[f32], threshold: f32) -> Vec<(i64, f32)> {
        let query_norm = query.iter().map(|value| value * value).sum::<f32>().sqrt();
        if query_norm == 0.0 {
            return Vec::new();
        }

        let mut hits: Vec<(i64, f32)> = Vec::new();
        for (row, &id) in self.ids.iter().enumerate() {
            let norm = self.norms[row];
            if norm == 0.0 {
                continue;
            }
            let start = row * self.dim;
            let vector = &self.data[start..start + self.dim];
            let mut dot = 0.0_f32;
            for (q, v) in query.iter().zip(vector.iter()) {
                dot += q * v;
            }
            let score = dot / (query_norm * norm);
            if score > threshold {
                hits.push((id, score));
            }
        }

        hits.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        hits
    }
}

#[derive(Default)]
pub struct VectorCacheState {
    cache: Arc<RwLock<Option<VectorCache>>>,
    building: Arc<AtomicBool>,
}

impl VectorCacheState {
    /// Drop the cached vectors. Called when the active library changes.
    pub fn invalidate(&self) {
        if let Ok(mut slot) = self.cache.write() {
            *slot = None;
        }
    }
}

fn enabled() -> bool {
    !matches!(
        std::env::var("LAP_VECTOR_CACHE").as_deref(),
        Ok("0") | Ok("false") | Ok("off")
    )
}

/// `(library key, embedded row count, largest embedded file id)`.
fn fingerprint() -> Result<(String, i64, i64), String> {
    let key = t_storage::get_current_db_path()?;
    let (count, max_id) = t_sqlite::embedding_fingerprint()?;
    Ok((key, count, max_id))
}

/// Read every embedding into one slab. Rows whose width differs from the
/// majority are dropped — a library part-way through a model change can hold
/// mixed widths, and those rows score 0 in the database path anyway.
fn build(library_key: String, fingerprint: (i64, i64)) -> Result<Option<VectorCache>, String> {
    let rows = t_sqlite::all_embeddings()?;
    if rows.len() < MIN_CACHED || rows.len() > MAX_CACHED {
        return Ok(None);
    }

    let mut widths: HashMap<usize, usize> = HashMap::new();
    for (_, vector) in &rows {
        *widths.entry(vector.len()).or_insert(0) += 1;
    }
    let Some(dim) = widths
        .into_iter()
        .max_by_key(|&(_, count)| count)
        .map(|(dim, _)| dim)
        .filter(|&dim| dim > 0)
    else {
        return Ok(None);
    };

    let started = std::time::Instant::now();
    let mut ids = Vec::with_capacity(rows.len());
    let mut data = Vec::with_capacity(rows.len() * dim);
    let mut norms = Vec::with_capacity(rows.len());
    for (id, vector) in rows {
        if vector.len() != dim {
            continue;
        }
        norms.push(vector.iter().map(|value| value * value).sum::<f32>().sqrt());
        data.extend_from_slice(&vector);
        ids.push(id);
    }
    if ids.len() < MIN_CACHED {
        return Ok(None);
    }

    println!(
        "vectors: cached {} embeddings (dim {}, {:.1} MB) in {} ms",
        ids.len(),
        dim,
        (data.len() * 4) as f64 / (1024.0 * 1024.0),
        started.elapsed().as_millis()
    );

    Ok(Some(VectorCache {
        library_key,
        dim,
        fingerprint,
        ids,
        data,
        norms,
    }))
}

/// Every `(file id, cosine similarity)` scoring strictly above `threshold`,
/// best first, or `Ok(None)` when the cache declined and the caller should
/// scan the database instead.
///
/// The returned ids are unfiltered: folder search-exclusion, inaccessible
/// albums and file-type filters all change without touching `afiles.embeds`,
/// so the caller applies them afterwards.
pub fn candidates(
    state: &tauri::State<'_, VectorCacheState>,
    query: &[f32],
    threshold: f32,
) -> Result<Option<Vec<(i64, f32)>>, String> {
    if !enabled() || query.is_empty() {
        return Ok(None);
    }

    let (library_key, count, max_id) = fingerprint()?;
    if (count as usize) < MIN_CACHED || (count as usize) > MAX_CACHED {
        return Ok(None);
    }

    let usable = {
        let guard = state
            .cache
            .read()
            .map_err(|_| "Vector cache lock poisoned".to_string())?;
        guard.as_ref().is_some_and(|cache| {
            cache.library_key == library_key
                && cache.fingerprint == (count, max_id)
                && cache.dim == query.len()
        })
    };

    if !usable {
        // One builder at a time; a concurrent search scans the database
        // rather than queueing behind the rebuild.
        if state
            .building
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Ok(None);
        }
        let outcome = match build(library_key.clone(), (count, max_id)) {
            Ok(Some(cache)) => {
                let matches_query = cache.dim == query.len();
                if let Ok(mut slot) = state.cache.write() {
                    *slot = Some(cache);
                }
                matches_query
            }
            Ok(None) => {
                if let Ok(mut slot) = state.cache.write() {
                    *slot = None;
                }
                false
            }
            Err(e) => {
                eprintln!("vectors: build failed, scanning the database: {}", e);
                false
            }
        };
        state.building.store(false, Ordering::SeqCst);
        if !outcome {
            return Ok(None);
        }
    }

    let guard = state
        .cache
        .read()
        .map_err(|_| "Vector cache lock poisoned".to_string())?;
    let Some(cache) = guard.as_ref() else {
        return Ok(None);
    };
    if cache.library_key != library_key || cache.dim != query.len() {
        return Ok(None);
    }

    Ok(Some(cache.search(query, threshold)))
}

#[cfg(test)]
mod tests_support {
    use super::*;
    use rusqlite::Connection;

    pub fn load(db: &str) -> Vec<(i64, Vec<f32>)> {
        let conn = Connection::open(db).expect("open library database");
        let mut stmt = conn
            .prepare("SELECT id, embeds FROM afiles WHERE embeds IS NOT NULL")
            .unwrap();
        let rows = stmt
            .query_map([], |row| {
                let id: i64 = row.get(0)?;
                let blob: Vec<u8> = row.get(1)?;
                Ok((id, blob))
            })
            .unwrap();
        rows.filter_map(Result::ok)
            .filter(|(_, blob)| !blob.is_empty() && blob.len() % 4 == 0)
            .map(|(id, blob)| {
                (
                    id,
                    blob.chunks_exact(4)
                        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                        .collect::<Vec<f32>>(),
                )
            })
            .collect()
    }

    pub fn build_test_cache(db: &str) -> VectorCache {
        let rows = load(db);
        let dim = rows[0].1.len();
        let rows: Vec<_> = rows.into_iter().filter(|(_, v)| v.len() == dim).collect();
        let mut ids = Vec::new();
        let mut data = Vec::new();
        let mut norms = Vec::new();
        for (id, vector) in &rows {
            norms.push(vector.iter().map(|value| value * value).sum::<f32>().sqrt());
            data.extend_from_slice(vector);
            ids.push(*id);
        }
        let max_id = *ids.iter().max().unwrap();
        VectorCache {
            library_key: db.to_string(),
            dim,
            fingerprint: (ids.len() as i64, max_id),
            ids,
            data,
            norms,
        }
    }

    /// The pre-cache behaviour: read and decode every BLOB per query.
    pub fn database_scan(db: &str, query: &[f32], threshold: f32) -> Vec<(i64, f32)> {
        let conn = Connection::open(db).unwrap();
        let mut stmt = conn
            .prepare("SELECT id, embeds FROM afiles WHERE embeds IS NOT NULL")
            .unwrap();
        let query_norm = query.iter().map(|value| value * value).sum::<f32>().sqrt();
        let rows = stmt
            .query_map([], |row| {
                let id: i64 = row.get(0)?;
                let blob: Vec<u8> = row.get(1)?;
                Ok((id, blob))
            })
            .unwrap();
        let mut hits = Vec::new();
        for row in rows.flatten() {
            let (id, blob) = row;
            if blob.len() % 4 != 0 || blob.len() / 4 != query.len() {
                continue;
            }
            let mut dot = 0.0_f32;
            let mut norm_squared = 0.0_f32;
            for (index, chunk) in blob.chunks_exact(4).enumerate() {
                let value = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                dot += query[index] * value;
                norm_squared += value * value;
            }
            let norm = norm_squared.sqrt();
            let score = if norm == 0.0 || query_norm == 0.0 {
                0.0
            } else {
                dot / (query_norm * norm)
            };
            if score > threshold {
                hits.push((id, score));
            }
        }
        hits.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        hits
    }
}

#[cfg(test)]
mod tests {
    use super::tests_support::load;
    use super::*;
    use std::collections::HashSet;

    /// The same arithmetic `AFile::cosine_similarity_blob` performs, so the
    /// test compares the cache against the production database path rather
    /// than against an idealised reference.
    fn cosine(query: &[f32], query_norm: f32, other: &[f32]) -> f32 {
        let mut dot = 0.0_f32;
        let mut other_norm_squared = 0.0_f32;
        for (index, value) in other.iter().enumerate() {
            dot += query[index] * value;
            other_norm_squared += value * value;
        }
        let other_norm = other_norm_squared.sqrt();
        if other_norm == 0.0 || query_norm == 0.0 {
            0.0
        } else {
            dot / (query_norm * other_norm)
        }
    }


    /// The cached scan must reproduce the database scan exactly — same ids,
    /// same scores, same order — at every threshold the app actually uses.
    /// Point `LAP_TEST_DB` at a real library to run it.
    #[test]
    fn cache_matches_database_scan() {
        let Ok(db) = std::env::var("LAP_TEST_DB") else {
            eprintln!("LAP_TEST_DB not set; skipping");
            return;
        };
        let rows = load(&db);
        assert!(
            rows.len() >= MIN_CACHED,
            "need a real library, got {} embeddings",
            rows.len()
        );
        let dim = rows[0].1.len();
        let rows: Vec<_> = rows.into_iter().filter(|(_, v)| v.len() == dim).collect();

        let mut ids = Vec::new();
        let mut data = Vec::new();
        let mut norms = Vec::new();
        for (id, vector) in &rows {
            norms.push(vector.iter().map(|value| value * value).sum::<f32>().sqrt());
            data.extend_from_slice(vector);
            ids.push(*id);
        }
        let cache = VectorCache {
            library_key: db.clone(),
            dim,
            fingerprint: (rows.len() as i64, *ids.iter().max().unwrap()),
            ids,
            data,
            norms,
        };

        let step = (rows.len() / 12).max(1);
        let mut checked = 0;
        for threshold in [0.9_f32, 0.8, 0.7, 0.32, 0.26, 0.25] {
            for (i, (_, query)) in rows.iter().enumerate().step_by(step) {
                let query_norm = query.iter().map(|value| value * value).sum::<f32>().sqrt();
                let mut expected: Vec<(i64, f32)> = rows
                    .iter()
                    .filter_map(|(id, v)| {
                        let score = cosine(query, query_norm, v);
                        (score > threshold).then_some((*id, score))
                    })
                    .collect();
                expected
                    .sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

                let actual = cache.search(query, threshold);

                let got: HashSet<i64> = actual.iter().map(|(id, _)| *id).collect();
                let want: HashSet<i64> = expected.iter().map(|(id, _)| *id).collect();
                assert!(
                    got == want,
                    "row {} at threshold {}: missing {:?}, extra {:?}",
                    i,
                    threshold,
                    want.difference(&got).collect::<Vec<_>>(),
                    got.difference(&want).collect::<Vec<_>>()
                );
                assert_eq!(
                    actual.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
                    expected.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
                    "row {} at threshold {}: ranking differs",
                    i,
                    threshold
                );
                for ((_, a), (_, b)) in actual.iter().zip(expected.iter()) {
                    assert!((a - b).abs() < 1e-6, "score {} vs {}", a, b);
                }
                checked += 1;
            }
        }
        println!(
            "cache_matches_database_scan: {} queries agreed exactly",
            checked
        );
        assert!(checked > 0);
    }
}

#[cfg(test)]
mod bench {
    use super::tests_support::*;
    use super::*;
    use std::time::Instant;

    /// Compares one query against the cached slab with one query served the
    /// old way: read every BLOB from SQLite, decode it, then score.
    #[test]
    fn cache_versus_database_scan_cost() {
        let Ok(db) = std::env::var("LAP_TEST_DB") else {
            eprintln!("LAP_TEST_DB not set; skipping");
            return;
        };
        let cache = build_test_cache(&db);
        let query: Vec<f32> = {
            let start = 0;
            cache.data[start..start + cache.dim].to_vec()
        };
        let threshold = 0.26_f32;
        const ROUNDS: u32 = 20;

        // Warm the page cache so the comparison is not measuring cold I/O.
        let _ = database_scan(&db, &query, threshold);
        let _ = cache.search(&query, threshold);

        let started = Instant::now();
        let mut db_hits = 0;
        for _ in 0..ROUNDS {
            db_hits = database_scan(&db, &query, threshold).len();
        }
        let db_us = started.elapsed().as_micros() / ROUNDS as u128;

        let started = Instant::now();
        let mut cache_hits = 0;
        for _ in 0..ROUNDS {
            cache_hits = cache.search(&query, threshold).len();
        }
        let cache_us = started.elapsed().as_micros() / ROUNDS as u128;

        assert_eq!(db_hits, cache_hits, "the two paths disagree");
        println!(
            "{} embeddings, {} hits: database {} us/query, cache {} us/query ({:.1}x)",
            cache.ids.len(),
            cache_hits,
            db_us,
            cache_us,
            db_us as f64 / cache_us.max(1) as f64
        );
    }
}
