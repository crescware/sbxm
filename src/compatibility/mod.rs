//! Docker Sandboxes CLIの出力を解釈する。
//!
//! 解釈できない出力から状態を推測しない。parseできない出力はerrorとして扱う。
//! 1 moduleが1 commandの出力を担当し、structured outputの共通部分だけを`json`が持つ。

mod daemon;
mod image;
mod json;
mod login;
mod policy;
mod sandbox;
mod secret;
mod template;
mod version;

pub use daemon::{DaemonState, parse_daemon_status};
pub use image::{ImageIdentity, parse_image_inspect};
pub use login::parse_login_status;
pub use policy::{EXPECTED_NETWORK_POLICY, parse_network_policy};
pub use sandbox::{SandboxEntry, SandboxState, parse_sandbox_list};
pub use secret::{CustomSecret, parse_custom_secrets};
pub use template::{TemplateEntry, parse_template_list};
pub use version::{CliVersion, require_minimum_version};
