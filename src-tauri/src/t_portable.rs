//! Telling a portable copy apart from an installed one.
//!
//! The portable build is a folder you unzip and run. The updater, on the other
//! hand, downloads an installer and runs it, which would put a *second*,
//! installed copy on the machine and leave the folder the user actually
//! launches untouched. So the portable build must not install updates in
//! place; it should send the user to the download page instead.
//!
//! Detection is deliberately explicit rather than clever: the portable archive
//! ships a `portable.txt` next to the executable, and nothing else does. No
//! registry probing, no guessing from the install path, and a user who copies
//! an installed build somewhere odd is not mistaken for a portable one.

use std::path::PathBuf;

const MARKER: &str = "portable.txt";

/// The marker file that would sit next to the running executable.
fn marker_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    Some(exe.parent()?.join(MARKER))
}

/// Whether this copy was launched from a portable folder.
pub fn is_portable() -> bool {
    marker_path().map(|path| path.is_file()).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A test binary has no marker beside it, so this is the installed answer.
    /// The interesting half — a marker present — is covered by the fact that
    /// `is_file()` is the whole rule; there is no parsing to get wrong.
    #[test]
    fn a_plain_build_is_not_portable() {
        assert!(!is_portable());
    }

    #[test]
    fn the_marker_sits_beside_the_executable() {
        let path = marker_path().expect("current_exe should resolve in a test");
        assert_eq!(path.file_name().unwrap(), MARKER);
        assert_eq!(
            path.parent().unwrap(),
            std::env::current_exe().unwrap().parent().unwrap()
        );
    }
}
