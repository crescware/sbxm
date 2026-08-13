//! 管理案件とSandboxの突き合わせ。
//!
//! runtime状態を複製せず、command実行のたびに1回のSandbox一覧取得から組み立てる。
//! 名前は完全一致で突き合わせ、substringや表示textの検索は使わない。

mod duplicated;
mod managed_project;
mod not_created;
mod observe;
mod observe_workspace;
mod observed;
mod poll;
mod project_state;
mod remove;
mod require_workspace;
mod single;
mod snapshot;
mod start;
mod state_of;
mod still_present;
mod take;
mod wait_until_running;
mod workspace_state;

use duplicated::duplicated;
pub use managed_project::ManagedProject;
pub use not_created::not_created;
use observe::observe;
pub use observe_workspace::observe_workspace;
pub use observed::Observed;
pub use poll::Poll;
pub use project_state::ProjectState;
pub use remove::remove;
use require_workspace::require_workspace;
pub use single::single;
pub use snapshot::Snapshot;
pub use start::start;
pub use state_of::state_of;
pub use still_present::still_present;
pub use take::take;
pub use wait_until_running::wait_until_running;
pub use workspace_state::WorkspaceState;

#[cfg(test)]
#[path = "inventory_test.rs"]
mod inventory_test;
