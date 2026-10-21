//! Docker Sandboxes CLIを使うcommandが、最初の`sbx`呼び出しより前に通す確認。
//!
//! 求める最低versionは`boundary::host::protocol`の`MINIMUM_CLI_VERSION`の1か所で決める。

mod require_ready;
mod require_supported_version;

pub use require_ready::require_ready;
pub use require_supported_version::require_supported_version;

#[cfg(test)]
#[path = "sbx_cli_test.rs"]
mod sbx_cli_test;
