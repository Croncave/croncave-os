//! Per-run snapshots: what a run created, changed or deleted, and a copy of anything it
//! deleted so Trash can catch it.
//!
//! The architecture calls for a copy-on-write filesystem. Locally we get the same effect
//! with a tree of hard links: taking it is cheap, and a file the run deletes stays alive
//! through its link.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use croncave_proto::{ChangeKind, FileChange};

use crate::disk::rel_string;

/// Folders whose churn isn't the person's work.
const IGNORED: &[&str] = &["node_modules", "__pycache__", ".git", ".venv"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stamp {
    pub size: u64,
    pub modified_ns: u128,
    pub is_dir: bool,
}

pub type Manifest = BTreeMap<String, Stamp>;

pub struct Snapshot {
    pub dir: PathBuf,
    pub manifest: Manifest,
}

pub fn manifest(root: &Path) -> Manifest {
    let mut out = Manifest::new();
    walk(root, root, &mut out);
    out
}

fn walk(root: &Path, dir: &Path, out: &mut Manifest) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let path = e.path();
        let Ok(meta) = std::fs::symlink_metadata(&path) else { continue };
        let name = e.file_name().to_string_lossy().into_owned();
        if meta.is_dir() && IGNORED.contains(&name.as_str()) {
            continue;
        }
        let modified_ns =
            meta.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_nanos()).unwrap_or(0);
        out.insert(rel_string(root, &path), Stamp { size: meta.len(), modified_ns, is_dir: meta.is_dir() });
        if meta.is_dir() {
            walk(root, &path, out);
        }
    }
}

/// Take a snapshot of `root` into `dir`.
pub fn take(root: &Path, dir: &Path) -> std::io::Result<Snapshot> {
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir)?;
    let manifest = manifest(root);
    for (rel, stamp) in &manifest {
        let dst = dir.join(rel);
        if stamp.is_dir {
            std::fs::create_dir_all(&dst)?;
        } else {
            if let Some(p) = dst.parent() {
                std::fs::create_dir_all(p)?;
            }
            let src = root.join(rel);
            // Hard links keep a deleted file alive; copy when the filesystem can't link.
            if std::fs::hard_link(&src, &dst).is_err() {
                let _ = std::fs::copy(&src, &dst);
            }
        }
    }
    Ok(Snapshot { dir: dir.to_path_buf(), manifest })
}

/// Files created, changed and deleted between two manifests. Deleted folders are
/// reported once, at the highest folder deleted.
pub fn diff(before: &Manifest, after: &Manifest) -> Vec<FileChange> {
    let mut out = Vec::new();
    for (path, a) in after {
        if a.is_dir {
            continue;
        }
        match before.get(path) {
            None => out.push(FileChange { path: path.clone(), kind: ChangeKind::Created, size: a.size }),
            Some(b) if b.size != a.size || b.modified_ns != a.modified_ns => {
                out.push(FileChange { path: path.clone(), kind: ChangeKind::Changed, size: a.size })
            }
            _ => {}
        }
    }
    for (path, b) in before {
        if after.contains_key(path) {
            continue;
        }
        let parent_gone = parent_paths(path).any(|p| before.contains_key(p) && !after.contains_key(p));
        if !parent_gone {
            out.push(FileChange { path: path.clone(), kind: ChangeKind::Deleted, size: b.size });
        }
    }
    out
}

fn parent_paths(path: &str) -> impl Iterator<Item = &str> {
    path.match_indices('/').map(move |(i, _)| &path[..i])
}

impl Snapshot {
    /// Compare with the disk now, move what the run deleted into Trash (attributed to the
    /// run) and release the snapshot.
    pub fn finish(self, disk: &crate::disk::Disk, run_by: &str) -> Vec<FileChange> {
        let after = manifest(&disk.root());
        let changes = diff(&self.manifest, &after);
        for c in changes.iter().filter(|c| c.kind == ChangeKind::Deleted) {
            let kept = self.dir.join(&c.path);
            if kept.exists()
                && let Err(e) = crate::files::adopt_into_trash(disk, &kept, &c.path, run_by)
            {
                tracing::warn!(path = %c.path, error = %e, "could not keep a deleted file in Trash");
            }
        }
        let _ = std::fs::remove_dir_all(&self.dir);
        changes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::disk::Disk;

    #[test]
    fn diff_reports_created_changed_and_top_level_deletes() {
        let s = |size| Stamp { size, modified_ns: 1, is_dir: false };
        let d = Stamp { size: 0, modified_ns: 1, is_dir: true };
        let before: Manifest = [
            ("keep.txt".to_string(), s(1)),
            ("edit.txt".to_string(), s(1)),
            ("old".to_string(), d),
            ("old/a.txt".to_string(), s(1)),
            ("gone.txt".to_string(), s(3)),
        ]
        .into();
        let after: Manifest =
            [("keep.txt".to_string(), s(1)), ("edit.txt".to_string(), s(2)), ("new.txt".to_string(), s(5))].into();
        let mut got: Vec<(String, ChangeKind)> = diff(&before, &after).into_iter().map(|c| (c.path, c.kind)).collect();
        got.sort();
        assert_eq!(
            got,
            vec![
                ("edit.txt".into(), ChangeKind::Changed),
                ("gone.txt".into(), ChangeKind::Deleted),
                ("new.txt".into(), ChangeKind::Created),
                ("old".into(), ChangeKind::Deleted),
            ]
        );
    }

    #[test]
    fn a_file_deleted_by_a_run_lands_in_trash() {
        let dir = tempfile::tempdir().unwrap();
        let disk = Disk::open(dir.path()).unwrap();
        std::fs::write(disk.root().join("report.csv"), "a,b\n1,2\n").unwrap();
        let snap = take(&disk.root(), &disk.system("snapshots").join("r1")).unwrap();
        std::fs::remove_file(disk.root().join("report.csv")).unwrap();
        std::fs::write(disk.root().join("out.txt"), "hi").unwrap();
        let changes = snap.finish(&disk, "run:r1");
        assert_eq!(changes.len(), 2);
        let trash = crate::files::trash_list(&disk);
        assert_eq!(trash.len(), 1);
        assert_eq!(trash[0].original_path, "report.csv");
        assert_eq!(trash[0].deleted_by, "run:r1");
        assert!(!disk.system("snapshots").join("r1").exists());
    }
}
