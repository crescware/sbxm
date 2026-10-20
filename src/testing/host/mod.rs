//! Sandboxを持つhostのfake。

mod answered_host;
mod assert_lifecycle;
mod changing_before;
mod custom_secret_listing;
mod failing_at;
mod fake_sbx;
mod is_sandbox_removal;
mod isolated_agent;
mod no_custom_secrets;
mod no_secrets;
mod registered_secret;
mod removal_command;
mod supported_git_version;
mod supported_sbx_version;
mod unrunnable;

pub use answered_host::AnsweredHost;
pub use assert_lifecycle::assert_lifecycle;
pub use changing_before::ChangingBefore;
pub use custom_secret_listing::custom_secret_listing;
pub use failing_at::FailingAt;
pub use fake_sbx::FakeSbx;
pub use is_sandbox_removal::is_sandbox_removal;
pub use isolated_agent::isolated_agent;
pub use no_custom_secrets::no_custom_secrets;
pub use no_secrets::no_secrets;
pub use registered_secret::registered_secret;
pub use removal_command::removal_command;
pub use supported_git_version::supported_git_version;
pub use supported_sbx_version::supported_sbx_version;
pub use unrunnable::Unrunnable;
