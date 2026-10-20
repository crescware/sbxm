//! hostのgitを使うcommandが、最初のgit呼び出しより前に通す確認。
//!
//! 求める最低versionは`MINIMUM_HOST_GIT`の1か所で決める。

mod minimum_host_git;
mod require_supported_version;

pub use minimum_host_git::MINIMUM_HOST_GIT;
pub use require_supported_version::require_supported_version;

#[cfg(test)]
#[path = "git_cli_test.rs"]
mod git_cli_test;
