//! `sbx daemon status`の解釈。

mod daemon_state;
mod parse_daemon_status;

pub use daemon_state::DaemonState;
pub use parse_daemon_status::parse_daemon_status;

#[cfg(test)]
#[path = "daemon_test.rs"]
mod daemon_test;
