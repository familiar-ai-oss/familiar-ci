//! A laptop-only marker, outside the repository, that this project was set up,
//! so removing `.familiar` from the working tree cannot make the project read as
//! never set up. It holds no criterion state, and a missing marker refuses nothing.

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// The folder under the data folder that holds one marker per project.
pub const PROJECTS: &str = "projects";

/// The file under the data folder that keeps this machine's choice about sharing gap
/// recordings (plan A-6 [draft]): one per machine, beside the per-project markers.
pub const SHARE: &str = "share";

/// The share choice's path under the data folder.
pub fn share_path(data_dir: &Path) -> PathBuf {
    data_dir.join(SHARE)
}

/// The project root, canonical when the path resolves and as given otherwise.
fn canonical(root: &Path) -> PathBuf {
    std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf())
}

/// The marker's path for a project root, under the data folder.
pub fn path(data_dir: &Path, root: &Path) -> PathBuf {
    let hash = Sha256::digest(canonical(root).to_string_lossy().as_bytes());
    let hex: String = hash.iter().map(|b| format!("{b:02x}")).collect();
    data_dir.join(PROJECTS).join(hex)
}

/// Whether the marker for this project root is present; no data folder means not.
pub fn present(data_dir: Option<&Path>, root: &Path) -> bool {
    data_dir.is_some_and(|d| path(d, root).is_file())
}

/// What `familiar-ai setup` did about the marker.
#[derive(Debug, PartialEq, Eq)]
pub enum Marked {
    Written(PathBuf),
    Already(PathBuf),
    NoHome,
}

/// The marker's text: the root path and the time, one line each.
pub fn text(root: &Path, now: f64) -> String {
    let seconds = if now.is_finite() && now >= 0.0 { now as u64 } else { 0 };
    format!("root: {}\nat: {seconds}\n", canonical(root).display())
}

/// Write the marker for this project root unless it is already there.
pub fn write(data_dir: Option<&Path>, root: &Path, now: f64) -> Result<Marked, String> {
    let Some(data_dir) = data_dir else {
        return Ok(Marked::NoHome);
    };
    let marker = path(data_dir, root);
    if marker.is_file() {
        return Ok(Marked::Already(marker));
    }
    let folder = data_dir.join(PROJECTS);
    std::fs::create_dir_all(&folder).map_err(|e| format!("cannot make {}: {e}", folder.display()))?;
    std::fs::write(&marker, text(root, now)).map_err(|e| format!("cannot write {}: {e}", marker.display()))?;
    Ok(Marked::Written(marker))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("familiar-ai-marker-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch");
        dir
    }

    #[test]
    fn the_marker_is_the_roots_hash_under_projects_and_holds_the_root_and_the_time() {
        let data = scratch("data");
        let root = scratch("root");
        assert!(!present(Some(&data), &root));
        assert_eq!(write(Some(&data), &root, 1759700000.6).expect("write"), Marked::Written(path(&data, &root)));
        assert!(present(Some(&data), &root));
        let p = path(&data, &root);
        assert_eq!(p.parent().and_then(Path::file_name).and_then(|n| n.to_str()), Some(PROJECTS));
        let name = p.file_name().and_then(|n| n.to_str()).expect("name");
        assert_eq!(name.len(), 64);
        assert!(name.chars().all(|c| c.is_ascii_hexdigit()));
        let body = std::fs::read_to_string(&p).expect("read");
        assert_eq!(body, format!("root: {}\nat: 1759700000\n", std::fs::canonicalize(&root).expect("canon").display()));
        // the same root through a different spelling is the same marker
        let dotted = root.join(".");
        assert_eq!(path(&data, &dotted), p);
        assert!(present(Some(&data), &dotted));
        // a second setup is idempotent: already there, the text untouched
        assert_eq!(write(Some(&data), &root, 2.0).expect("again"), Marked::Already(p.clone()));
        assert_eq!(std::fs::read_to_string(&p).expect("read"), body);
        // another root is another marker
        let other = scratch("other");
        assert_ne!(path(&data, &other), p);
        assert!(!present(Some(&data), &other));
        let _ = std::fs::remove_dir_all(&data);
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&other);
    }

    #[test]
    fn no_home_means_no_marker_written_and_none_present() {
        let root = scratch("nohome");
        assert_eq!(write(None, &root, 1.0).expect("no home"), Marked::NoHome);
        assert!(!present(None, &root));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_data_folder_comes_from_the_three_names_in_order() {
        use crate::Surroundings;
        assert_eq!(Surroundings::data_dir_from(Some("/f"), Some("/x"), Some("/h")), Some(PathBuf::from("/f")));
        assert_eq!(Surroundings::data_dir_from(None, Some("/x"), Some("/h")), Some(PathBuf::from("/x/familiar-ai")));
        assert_eq!(Surroundings::data_dir_from(Some(""), None, Some("/h")), Some(PathBuf::from("/h/.local/share/familiar-ai")));
        assert_eq!(Surroundings::data_dir_from(None, None, None), None);
        assert_eq!(Surroundings::data_dir_from(None, Some(""), Some("")), None);
    }
}
