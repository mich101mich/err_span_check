use std::path::{Path, PathBuf};

#[derive(Eq, PartialEq, Ord, PartialOrd, Clone)]
pub(crate) struct CanonicalPath(PathBuf);

impl CanonicalPath {
    pub(crate) fn new(path: &Path) -> Self {
        if let Ok(canonical) = path.canonicalize() {
            CanonicalPath(canonical)
        } else {
            CanonicalPath(path.to_owned())
        }
    }
}
