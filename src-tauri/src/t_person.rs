/**
 * Correcting who is who.
 *
 * Face clustering groups by resemblance, and resemblance is not identity. It
 * reliably splits one person across several groups — different lighting, a
 * decade apart, glasses on or off — and occasionally puts two people in one.
 * Without a way to say "these are the same person" or "that one isn't them",
 * the results can only be browsed, never corrected, which is why the feature
 * has stayed behind a Beta label.
 *
 * These are the three corrections that matter: merge groups that are one
 * person, move a single face to the right person, and detach a face that
 * belongs to nobody in the list. Everything here edits assignments only; the
 * detected faces and their embeddings are never touched, so a later
 * re-clustering pass has the same material to work from.
 */
use rusqlite::params;

use crate::t_sqlite::{open_conn, Person};

/// Fold `sources` into `target`: every face moves across and the now-empty
/// groups are removed. Returns how many faces were reassigned.
///
/// The whole thing runs in one transaction — a half-finished merge would
/// leave faces pointing at a person that no longer exists.
pub fn merge_persons(target_id: i64, source_ids: &[i64]) -> Result<usize, String> {
    let sources: Vec<i64> = source_ids
        .iter()
        .copied()
        .filter(|id| *id != target_id)
        .collect();
    if sources.is_empty() {
        return Ok(0);
    }

    let mut conn = open_conn()?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    // The target must exist, or the faces would be orphaned.
    let target_exists: i64 = tx
        .query_row(
            "SELECT COUNT(*) FROM persons WHERE id = ?1",
            params![target_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    if target_exists == 0 {
        return Err(format!("Person {} no longer exists", target_id));
    }

    let placeholders = std::iter::repeat("?")
        .take(sources.len())
        .collect::<Vec<_>>()
        .join(",");

    let moved;
    {
        let sql = format!(
            "UPDATE faces SET person_id = ?1 WHERE person_id IN ({})",
            placeholders
        );
        let mut values: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(target_id)];
        for id in &sources {
            values.push(Box::new(*id));
        }
        let refs: Vec<&dyn rusqlite::ToSql> = values.iter().map(|v| v.as_ref()).collect();
        moved = tx
            .execute(&sql, refs.as_slice())
            .map_err(|e| e.to_string())?;
    }

    {
        let sql = format!("DELETE FROM persons WHERE id IN ({})", placeholders);
        let refs: Vec<&dyn rusqlite::ToSql> = sources
            .iter()
            .map(|id| id as &dyn rusqlite::ToSql)
            .collect();
        tx.execute(&sql, refs.as_slice())
            .map_err(|e| e.to_string())?;
    }

    tx.commit().map_err(|e| e.to_string())?;

    // The target's cover may have come from a group that no longer exists.
    let _ = Person::update_thumbnail(target_id);
    Ok(moved)
}

/// Point one face at `person_id`, or at nobody when it is `None`.
///
/// Both the old and the new owner get a fresh cover, since either may have
/// been representing itself with the face that just moved.
pub fn assign_face(face_id: i64, person_id: Option<i64>) -> Result<(), String> {
    let conn = open_conn()?;

    let previous: Option<i64> = conn
        .query_row(
            "SELECT person_id FROM faces WHERE id = ?1",
            params![face_id],
            |row| row.get(0),
        )
        .map_err(|e| format!("Face {} not found: {}", face_id, e))?;

    if let Some(id) = person_id {
        let exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM persons WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        if exists == 0 {
            return Err(format!("Person {} no longer exists", id));
        }
    }

    conn.execute(
        "UPDATE faces SET person_id = ?1 WHERE id = ?2",
        params![person_id, face_id],
    )
    .map_err(|e| format!("Failed to reassign the face: {}", e))?;
    drop(conn);

    if let Some(id) = previous {
        let _ = Person::update_thumbnail(id);
    }
    if let Some(id) = person_id {
        let _ = Person::update_thumbnail(id);
    }
    Ok(())
}

/// Start a new person from one face — the way to split someone the clustering
/// merged into the wrong group. Returns the new person's id.
pub fn split_face_to_new_person(face_id: i64, name: Option<&str>) -> Result<i64, String> {
    let person_id = Person::create(name)?;
    assign_face(face_id, Some(person_id))?;
    Ok(person_id)
}

/// Remove a person that has no faces left. Called after corrections so an
/// emptied group does not linger in the list.
pub fn prune_empty_persons() -> Result<usize, String> {
    let conn = open_conn()?;
    conn.execute(
        "DELETE FROM persons
         WHERE NOT EXISTS (SELECT 1 FROM faces WHERE faces.person_id = persons.id)",
        [],
    )
    .map_err(|e| format!("Failed to remove empty people: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A test build resolves its data directory with a `.debug` suffix, so
    /// these run against their own database and never touch a real library.
    /// The gate is kept so the intent is explicit.
    fn enabled() -> bool {
        std::env::var("LAP_TEST_PERSONS").as_deref() == Ok("1")
    }

    struct Fixture {
        person_ids: Vec<i64>,
        face_ids: Vec<i64>,
    }

    /// Faces reference a real file, which in turn needs a folder and an album,
    /// because foreign keys are enforced. Build the smallest chain that holds.
    fn seed_file() -> Result<i64, String> {
        crate::t_sqlite::create_db()?;
        let conn = open_conn()?;

        if let Ok(id) = conn.query_row("SELECT id FROM afiles LIMIT 1", [], |row| {
            row.get::<_, i64>(0)
        }) {
            return Ok(id);
        }

        conn.execute(
            "INSERT INTO albums (name, path) VALUES ('lap-test-album', 'D:/lap-test')",
            [],
        )
        .map_err(|e| e.to_string())?;
        let album_id = conn.last_insert_rowid();

        conn.execute(
            "INSERT INTO afolders (album_id, name, path) VALUES (?1, 'lap-test', 'D:/lap-test')",
            params![album_id],
        )
        .map_err(|e| e.to_string())?;
        let folder_id = conn.last_insert_rowid();

        conn.execute(
            "INSERT INTO afiles (folder_id, name, size) VALUES (?1, 'lap-test.jpg', 1)",
            params![folder_id],
        )
        .map_err(|e| e.to_string())?;
        Ok(conn.last_insert_rowid())
    }

    /// Three people with two faces each.
    fn seed() -> Result<Fixture, String> {
        let file_id = seed_file()?;
        let conn = open_conn()?;

        let mut person_ids = Vec::new();
        let mut face_ids = Vec::new();
        for index in 0..3 {
            conn.execute(
                "INSERT INTO persons (name, created_at) VALUES (?1, 0)",
                params![format!("lap-test-person-{}", index)],
            )
            .map_err(|e| e.to_string())?;
            let person_id = conn.last_insert_rowid();
            person_ids.push(person_id);

            for _ in 0..2 {
                conn.execute(
                    "INSERT INTO faces (file_id, bbox, person_id, created_at)
                     VALUES (?1, '{}', ?2, 0)",
                    params![file_id, person_id],
                )
                .map_err(|e| e.to_string())?;
                face_ids.push(conn.last_insert_rowid());
            }
        }
        Ok(Fixture { person_ids, face_ids })
    }

    fn cleanup(fixture: &Fixture) {
        let Ok(conn) = open_conn() else { return };
        for id in &fixture.face_ids {
            let _ = conn.execute("DELETE FROM faces WHERE id = ?1", params![id]);
        }
        for id in &fixture.person_ids {
            let _ = conn.execute("DELETE FROM persons WHERE id = ?1", params![id]);
        }
        let _ = conn.execute(
            "DELETE FROM persons WHERE name LIKE 'lap-test-person-%'",
            [],
        );
    }

    fn faces_of(person_id: i64) -> i64 {
        let conn = open_conn().unwrap();
        conn.query_row(
            "SELECT COUNT(*) FROM faces WHERE person_id = ?1",
            params![person_id],
            |row| row.get(0),
        )
        .unwrap_or(0)
    }

    fn person_exists(person_id: i64) -> bool {
        let conn = open_conn().unwrap();
        conn.query_row(
            "SELECT COUNT(*) FROM persons WHERE id = ?1",
            params![person_id],
            |row| row.get::<_, i64>(0),
        )
        .unwrap_or(0)
            > 0
    }

    #[test]
    fn merging_moves_every_face_and_removes_the_source() {
        if !enabled() {
            eprintln!("LAP_TEST_PERSONS not set; skipping");
            return;
        }
        let fixture = seed().expect("seed");
        let (target, a, b) = (
            fixture.person_ids[0],
            fixture.person_ids[1],
            fixture.person_ids[2],
        );

        let moved = merge_persons(target, &[a, b]).expect("merge");
        assert_eq!(moved, 4, "both sources had two faces each");
        assert_eq!(faces_of(target), 6);
        assert!(!person_exists(a), "merged-away person should be gone");
        assert!(!person_exists(b));

        cleanup(&fixture);
    }

    #[test]
    fn merging_into_itself_changes_nothing() {
        if !enabled() {
            eprintln!("LAP_TEST_PERSONS not set; skipping");
            return;
        }
        let fixture = seed().expect("seed");
        let target = fixture.person_ids[0];

        assert_eq!(merge_persons(target, &[target]).expect("merge"), 0);
        assert_eq!(faces_of(target), 2);
        assert!(person_exists(target));

        cleanup(&fixture);
    }

    #[test]
    fn a_face_can_be_moved_and_detached() {
        if !enabled() {
            eprintln!("LAP_TEST_PERSONS not set; skipping");
            return;
        }
        let fixture = seed().expect("seed");
        let (from, to) = (fixture.person_ids[0], fixture.person_ids[1]);
        let face = fixture.face_ids[0];

        assign_face(face, Some(to)).expect("reassign");
        assert_eq!(faces_of(from), 1);
        assert_eq!(faces_of(to), 3);

        assign_face(face, None).expect("detach");
        assert_eq!(faces_of(to), 2, "the face now belongs to nobody");

        cleanup(&fixture);
    }

    #[test]
    fn splitting_a_face_creates_a_person_for_it() {
        if !enabled() {
            eprintln!("LAP_TEST_PERSONS not set; skipping");
            return;
        }
        let mut fixture = seed().expect("seed");
        let original = fixture.person_ids[0];
        let face = fixture.face_ids[0];

        let new_id = split_face_to_new_person(face, Some("lap-test-person-split")).expect("split");
        fixture.person_ids.push(new_id);

        assert_eq!(faces_of(new_id), 1);
        assert_eq!(faces_of(original), 1);
        assert!(person_exists(new_id));

        cleanup(&fixture);
    }

    #[test]
    fn merging_into_a_person_that_is_gone_is_refused() {
        if !enabled() {
            eprintln!("LAP_TEST_PERSONS not set; skipping");
            return;
        }
        let fixture = seed().expect("seed");
        let source = fixture.person_ids[0];
        let missing = 9_000_000_i64;

        assert!(merge_persons(missing, &[source]).is_err());
        // The source must survive a refused merge.
        assert!(person_exists(source));
        assert_eq!(faces_of(source), 2);

        cleanup(&fixture);
    }
}
