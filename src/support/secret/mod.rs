//! `案件限定のGitHub` credential。
//!
//! tokenの発行と入力は自動化しない。存在確認だけをread-onlyで行い、値は取得も
//! 表示もしない。
//!
//! Sandboxの中へはtokenを渡さない。`sbx secret set-custom`で登録したcustom secretは、
//! Sandboxにplaceholderだけを見せ、登録済みhost宛のrequestに現れたplaceholderをproxyが
//! 本物へ差し替える。service secretを使わないのは、proxyのgithub presetがtokenの形で
//! 扱いを変え、classic personal access tokenを注入しないためである。

mod configure_git_credential;
mod forget_command;
mod forget_github;
mod github_host;
mod github_hosts;
mod github_token_env;
mod list_customs;
mod placeholder_probe;
mod register_command;
mod require_github;
mod require_placeholder_present;

pub use configure_git_credential::configure_git_credential;
pub use forget_command::forget_command;
pub use forget_github::forget_github;
pub use github_host::GITHUB_HOST;
pub use github_hosts::GITHUB_HOSTS;
pub use github_token_env::GITHUB_TOKEN_ENV;
use list_customs::list_customs;
pub(crate) use placeholder_probe::placeholder_probe;
pub use register_command::register_command;
pub use require_github::require_github;
pub use require_placeholder_present::require_placeholder_present;

#[cfg(test)]
#[path = "secret_test.rs"]
mod secret_test;
