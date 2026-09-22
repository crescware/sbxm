//! `案件限定のGitHub` credential。
//!
//! tokenの発行と入力は自動化しない。存在確認だけをread-onlyで行い、値は取得も
//! 表示もしない。
//!
//! Sandboxの中へはtokenを渡さない。`sbx secret set-custom`で登録したcustom secretは、
//! 登録済みhost宛のrequestに現れたplaceholderを、proxyが本物のtokenへ差し替える。
//! service secretを使わないのは、proxyのgithub presetがtokenの形で扱いを変え、
//! classic personal access tokenを注入しないためである。実機では、classic tokenを
//! service secretとして登録したSandboxからのrequestが、外からは200、中からは401に
//! なった。custom secretはtokenの形を問わない。
//!
//! placeholderはSandboxの環境変数ではなく、gitのcredential helperへ直接持たせる。
//! Docker Sandboxesは`GH_TOKEN`を組み込み`github` serviceのために予約しており、
//! tokenを1件も保存していなくても、作成した各Sandboxのこの変数をserviceのsentinelで
//! 埋める。そのため同名のcustom secretのplaceholderはSandboxへ届かない。requestの
//! 中にplaceholderが現れさえすればproxyは差し替えるので、環境変数を当てにしない。

mod configure_git_credential;
mod configure_token_env;
mod covers_github_hosts;
mod credential_key;
mod expected_credential_helper;
mod expected_token_env;
mod forget_command;
mod forget_github;
mod github_host;
mod github_hosts;
mod github_token_env;
mod github_token_fallback_env;
mod helper_prefix;
mod is_global_scope;
mod is_sbxm_helper;
mod is_sbxm_token_env;
mod list_customs;
mod observe_git_credential;
mod observe_token_env;
mod register_command;
mod registered_github;
mod require_github;
mod require_github_accepts;
mod token_env_file;
mod token_env_marker;

pub use configure_git_credential::configure_git_credential;
pub use configure_token_env::configure_token_env;
use covers_github_hosts::covers_github_hosts;
use credential_key::credential_key;
use expected_credential_helper::expected_credential_helper;
use expected_token_env::expected_token_env;
pub use forget_command::forget_command;
pub use forget_github::forget_github;
pub use github_host::GITHUB_HOST;
pub use github_hosts::GITHUB_HOSTS;
pub use github_token_env::GITHUB_TOKEN_ENV;
use github_token_fallback_env::GITHUB_TOKEN_FALLBACK_ENV;
use helper_prefix::HELPER_PREFIX;
use is_global_scope::is_global_scope;
use is_sbxm_helper::is_sbxm_helper;
use is_sbxm_token_env::is_sbxm_token_env;
use list_customs::list_customs;
pub(crate) use observe_git_credential::observe_git_credential;
pub(crate) use observe_token_env::observe_token_env;
pub use register_command::register_command;
use registered_github::registered_github;
pub use require_github::require_github;
pub use require_github_accepts::require_github_accepts;
use token_env_file::TOKEN_ENV_FILE;
use token_env_marker::TOKEN_ENV_MARKER;

#[cfg(test)]
#[path = "secret_test.rs"]
mod secret_test;
