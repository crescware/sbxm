//! Docker Sandboxes daemonの観測。
//!
//! sbxmはdaemonを停止も起動もしない。daemonを止めるには動作中のSandboxを止める
//! 必要があり、作業中のSandboxを巻き込むためである。
//!
//! hostのSSH AgentがSandboxへ渡っていないことは、daemonの起動条件から推定せず、
//! 作成したSandboxの中から観測する（`sandbox::require_credentials_isolated`）。

mod list;
mod list_retrying;
mod list_with_timeout;

pub use list::list;
use list_retrying::list_retrying;
pub use list_with_timeout::list_with_timeout;

#[cfg(test)]
#[path = "daemon_test.rs"]
mod daemon_test;
