use std::path::Path;

use crate::diagnostics::{Result, unparseable};
use crate::paths;

/// `git worktree list --porcelain -z`の1件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub path: String,
    pub bare: bool,
    pub detached: bool,
    /// porcelainのHEAD。bareや未観測のrecordではNone。OIDを使う側は`head_oid`で検証する。
    pub head: Option<String>,
    /// checkoutしているbranchのref。`refs/heads/<branch>`の形。detachedとbareでは`None`。
    pub branch: Option<String>,
    /// worktreeのdirectoryが無いなど、gitが片付けてよいと見なしたもの。
    pub prunable: bool,
}

impl Entry {
    /// 保存・容量見積もりが共通で使う、完全な16進OID。
    pub fn head_oid(&self) -> Result<&str> {
        self.head
            .as_deref()
            .filter(|oid| crate::git::is_object_id(oid))
            .ok_or_else(|| unparseable("git worktree list", "missing or invalid worktree HEAD"))
    }

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
