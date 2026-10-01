//! The computer's disk: the person's root and the system area apps keep outside it.

use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Disk {
    base: PathBuf,
}

impl Disk {
    pub fn open(base: &Path) -> anyhow::Result<Self> {
        std::fs::create_dir_all(base.join("root"))?;
        std::fs::create_dir_all(base.join("system"))?;
        Ok(Self { base: base.canonicalize()? })
    }

    /// The person's own folder. Empty at first.
    pub fn root(&self) -> PathBuf {
        self.base.join("root")
    }

    /// A folder in the system area, created on demand.
    pub fn system(&self, sub: &str) -> PathBuf {
        let p = self.base.join("system").join(sub);
        let _ = std::fs::create_dir_all(&p);
        p
    }

    /// Resolve a person-facing path ("Reports/a.csv", "/Reports") inside the root.
    pub fn resolve(&self, rel: &str) -> Result<PathBuf, String> {
        Ok(self.root().join(clean(rel)?))
    }
}

/// Normalise a relative path, refusing anything that could leave the root.
pub fn clean(rel: &str) -> Result<PathBuf, String> {
    let mut out = PathBuf::new();
    for comp in Path::new(rel.trim_start_matches('/')).components() {
        match comp {
            Component::Normal(c) => out.push(c),
            Component::CurDir => {}
            _ => return Err(format!("\"{rel}\" isn't a path inside your files")),
        }
    }
    Ok(out)
}

/// A path relative to `base` with `/` separators.
pub fn rel_string(base: &Path, path: &Path) -> String {
    path.strip_prefix(base)
        .unwrap_or(path)
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

/// Total size of a file or folder.
pub fn size_of(path: &Path) -> u64 {
    match std::fs::symlink_metadata(path) {
        Ok(m) if m.is_dir() => {
            std::fs::read_dir(path).map(|rd| rd.flatten().map(|e| size_of(&e.path())).sum()).unwrap_or(0)
        }
        Ok(m) => m.len(),
        Err(_) => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_refuses_escapes() {
        assert_eq!(clean("/a/./b").unwrap(), PathBuf::from("a/b"));
        assert_eq!(clean("").unwrap(), PathBuf::new());
        assert!(clean("../etc/passwd").is_err());
        assert!(clean("a/../../b").is_err());
    }
}
