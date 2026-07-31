use std::path::Path;

use crate::paths;

/// `git worktree list --porcelain -z`の1件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub path: String,
    pub bare: bool,
    pub detached: bool,
}

impl Entry {
    /// bare root配下へstandardizeできる場合の相対path。
    ///
    /// bare entry、およびbare rootの外を指すpathは、この案件のworktreeではない。
    pub fn relative_to(&self, bare_root: &str) -> Option<String> {
        if self.bare {
            return None;
        }
        let standardized = paths::lexically_standardize(Path::new(&self.path));
        let relative = standardized.strip_prefix(bare_root).ok()?;
        Some(paths::display(relative))
    }
}
