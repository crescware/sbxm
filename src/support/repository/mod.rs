//! Sandbox内のbare repositoryとmanaged worktree。
//!
//! 1 Sandboxにつき1つのbare repositoryを持ち、作業用のworktreeをその下に並べる。
//! 1 treeの場合もbare repositoryとworktreeを分離する。

mod ensure_bare_clone;
mod fetch_refspec;
#[cfg(test)]
#[path = "mod_test.rs"]
mod mod_test;
mod refresh_origin;
mod start_ref;
mod unusable;
mod worktree;

pub use ensure_bare_clone::ensure_bare_clone;
pub(crate) use fetch_refspec::FETCH_REFSPEC;
pub use refresh_origin::refresh_origin;
pub use start_ref::resolve_start_ref;
use unusable::unusable;
pub use worktree::ensure_worktrees;
