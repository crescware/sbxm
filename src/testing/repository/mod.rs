//! `support::repository`のtestが共有するfixture。

mod canonical;
mod git_in;
mod healthy_clone;
mod layout;
mod metadata;
mod project;
mod project_paths;
mod worktree_host;

pub use canonical::canonical;
pub use git_in::git_in;
pub use healthy_clone::healthy_clone;
pub use layout::layout;
pub use metadata::metadata;
pub use project::project;
pub use project_paths::project_paths;
pub use worktree_host::worktree_host;
