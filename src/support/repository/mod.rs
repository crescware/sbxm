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
mod refresh_origin_all_refs;
mod start_ref;
mod tag_following;
mod unusable;
mod verify_bare_clone;
mod verify_existing;
mod worktree;

pub use ensure_bare_clone::ensure_bare_clone;
pub(crate) use fetch_refspec::FETCH_REFSPEC;
pub use refresh_origin::refresh_origin;
pub use refresh_origin_all_refs::refresh_origin_all_refs;
pub use start_ref::resolve_start_ref;
pub use tag_following::TagFollowing;
use unusable::unusable;
pub(crate) use verify_bare_clone::verify_bare_clone;
pub(crate) use verify_existing::verify_existing;
pub use worktree::ensure_worktrees;
