//! 全managed worktreeの起点にするbranchの決定。

mod remote_default_branch;
mod require_branch_name;
mod resolve_start_ref;

pub(super) use remote_default_branch::remote_default_branch;
pub(super) use require_branch_name::require_branch_name;
pub use resolve_start_ref::resolve_start_ref;

#[cfg(test)]
#[path = "start_ref_test.rs"]
mod start_ref_test;
