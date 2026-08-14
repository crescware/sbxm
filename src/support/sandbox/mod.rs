//! Sandboxの作成と再利用。
//!
//! Workspaceは案件pathともhomeとも無関係な中立pathとし、host側の実pathを
//! Sandboxへ公開しない。既存Sandboxは、作成元を問わず、期待する状態と一致することを
//! 観測できた場合だけ再利用する。

mod agent_kit;
mod ensure;
mod exec;
mod exec_arguments;
mod exec_as_root;
mod exec_with_progress;
mod find;
mod git_fatal;
mod inner_exit_code;
mod path_exists;
mod read;
mod ready_sandbox;
mod relayed;
mod remove_confirmed;
mod require_credentials_isolated;
mod run_exec;
mod ssh_add_no_agent;
mod ssh_agent_is_exposed;
mod unobservable;
mod unusable;
mod verify;
mod verify_identity;
mod workspace_exists;
mod workspace_path;
mod workspace_root;

use agent_kit::AGENT_KIT;
pub use ensure::ensure;
pub use exec::exec;
use exec_arguments::exec_arguments;
pub use exec_as_root::exec_as_root;
pub use exec_with_progress::exec_with_progress;
use find::find;
pub use git_fatal::GIT_FATAL;
pub use inner_exit_code::inner_exit_code;
pub use path_exists::path_exists;
pub use read::read;
pub use ready_sandbox::ReadySandbox;
pub use relayed::relayed;
pub use remove_confirmed::remove_confirmed;
pub use require_credentials_isolated::require_credentials_isolated;
use run_exec::run_exec;
pub use ssh_add_no_agent::SSH_ADD_NO_AGENT;
pub use ssh_agent_is_exposed::ssh_agent_is_exposed;
pub use unobservable::unobservable;
use unusable::unusable;
use verify::verify;
pub use verify_identity::verify_identity;
pub use workspace_exists::workspace_exists;
pub use workspace_path::workspace_path;
pub use workspace_root::WORKSPACE_ROOT;

#[cfg(test)]
#[path = "sandbox_test.rs"]
mod sandbox_test;
