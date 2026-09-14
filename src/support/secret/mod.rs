//! `案件限定のGitHub` credential。
//!
//! tokenの発行と入力は自動化しない。存在確認だけをread-onlyで行い、値は取得も
//! 表示もしない。
//!
//! Sandboxの中へはtokenを渡さない。Docker Sandboxesの組み込み`github` serviceへ
//! Sandbox限定で登録したtokenは、Sandboxには`GH_TOKEN`のsentinelとしてだけ見え、
//! github.com宛のrequestに現れたsentinelをproxyが本物へ差し替える。gitのBasic認証も
//! 同じ経路で差し替わることを実機で確かめている。
//!
//! 以前の版は`sbx secret set-custom --env GH_TOKEN`のcustom secretを案内していた。
//! しかしDocker Sandboxesは`GH_TOKEN`を組み込み`github` serviceのために予約しており、
//! 同名のcustom secretのplaceholderはSandboxへ届かない。Sandboxが持つのはserviceの
//! sentinelであり、serviceにtokenが無ければproxyは何も差し替えず、GitHubは
//! 「Invalid username or token」で拒む。その登録が残っている場合は、拒む理由として
//! 示し、後片付けの対象にも含める。

mod configure_git_credential;
mod credential_key;
mod expected_credential_helper;
mod forget_command;
mod forget_custom_command;
mod forget_github;
mod github_host;
mod github_service;
mod github_token_env;
mod list_secrets;
mod observe_git_credential;
mod placeholder_probe;
mod register_command;
mod require_github;
mod require_github_accepts;
mod require_token_env_present;

pub use configure_git_credential::configure_git_credential;
use credential_key::credential_key;
use expected_credential_helper::expected_credential_helper;
pub use forget_command::forget_command;
pub use forget_custom_command::forget_custom_command;
pub use forget_github::forget_github;
pub use github_host::GITHUB_HOST;
pub use github_service::GITHUB_SERVICE;
pub use github_token_env::GITHUB_TOKEN_ENV;
use list_secrets::list_secrets;
pub(crate) use observe_git_credential::observe_git_credential;
pub(crate) use placeholder_probe::placeholder_probe;
pub use register_command::register_command;
pub use require_github::require_github;
pub use require_github_accepts::require_github_accepts;
pub use require_token_env_present::require_token_env_present;

#[cfg(test)]
#[path = "secret_test.rs"]
mod secret_test;
