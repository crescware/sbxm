//! Sandbox内のbare repositoryとmanaged worktree。
//!
//! 1 Sandboxにつき1つのbare repositoryを持ち、作業用のworktreeをその下に並べる。
//! 1 treeの場合もbare repositoryとworktreeを分離する。

mod ensure_bare_clone;
mod fetch_refspec;
mod has_local_branches;
mod host_git;
mod host_repository;
#[cfg(test)]
#[path = "mod_test.rs"]
mod mod_test;
mod refresh_origin;
mod refresh_origin_all_refs;
mod sandbox_origin;
mod sandbox_remote;
mod sandbox_ssh_config;
mod sandbox_unreadable;
mod start_ref;
mod tag_following;
mod unusable;
mod verify_bare_clone;
mod worktree;

pub use ensure_bare_clone::ensure_bare_clone;
pub(crate) use fetch_refspec::FETCH_REFSPEC;
pub use has_local_branches::has_local_branches;
pub use host_git::host_git;
pub use host_repository::host_repository;
pub use refresh_origin::refresh_origin;
pub use refresh_origin_all_refs::refresh_origin_all_refs;
pub use sandbox_origin::SandboxOrigin;
pub use sandbox_remote::sandbox_remote;
pub use sandbox_ssh_config::sandbox_ssh_config;
pub use sandbox_unreadable::sandbox_unreadable;
pub use start_ref::resolve_start_ref;
pub use tag_following::TagFollowing;
use unusable::unusable;
pub use verify_bare_clone::verify_bare_clone;
pub use worktree::{adopt_worktree, ensure_worktrees};
