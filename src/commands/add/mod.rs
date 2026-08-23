//! `sbxm add`。

mod add_output;
mod add_request;
mod args;
mod ask_git_identity;
mod ask_language;
mod bundled_dockerfile;
mod choose_git_identity;
mod choose_language;
pub mod exec;
mod host_clone;
mod identity_prompt;
mod observe;
mod presence;
pub mod print;
mod prompt_ui;
mod register;
mod registration;
pub mod run;
mod target_configuration;
mod was_already_registered;

pub use add_output::AddOutput;
pub use add_request::AddRequest;
pub use args::Args;
pub(super) use ask_git_identity::ask_git_identity;
pub(super) use ask_language::ask_language;
use bundled_dockerfile::BUNDLED_DOCKERFILE;
pub(super) use choose_git_identity::choose_git_identity;
pub(super) use choose_language::choose_language;
pub use exec::exec;
pub use identity_prompt::IdentityPrompt;
use observe::observe;
use presence::Presence;
pub use register::register;
pub use registration::Registration;
pub use target_configuration::TargetConfiguration;
use was_already_registered::was_already_registered;
