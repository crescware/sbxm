//! Docker Sandboxes `CLIのversion`。
//!
//! `<major>.<minor>.<patch>`だけを読み、周囲の文字列からは意味を取らない。

mod cli_version;
mod minimum_cli_version;
mod require_minimum_version;

pub use cli_version::CliVersion;
pub use minimum_cli_version::MINIMUM_CLI_VERSION;
pub use require_minimum_version::require_minimum_version;

#[cfg(test)]
#[path = "version_test.rs"]
mod version_test;
