//! 外部protocolの出力を解釈する。Docker Sandboxes CLIが大半を占めるが、`df`のような
//! POSIX commandの出力も同じ分類軸へ置く。
//!
//! 解釈できない出力から状態を推測しない。parseできない出力はerrorとして扱う。
//! 1 moduleが1 commandの出力を担当し、structured outputの共通部分だけを`json`が持つ。
//!
//! `boundary::host`がprocessを実行してbytesを運ぶtransport境界であるのに対し、ここは
//! 外部toolのprotocol vocabularyを内部値へ変換するadapter群である。複数のsupport workflow
//! が同じprotocol型を共有するため、transportの下へ置くのではなく、protocolという別の
//! 分類軸を持つtop-level moduleとして維持する。

mod daemon;
mod disk;
mod image;
mod json;
mod login;
mod policy;
mod sandbox;
mod secret;
mod template;
mod version;

pub use daemon::{DaemonState, parse_daemon_status};
pub use disk::{RootDiskUsage, parse_df};
pub use image::{ImageIdentity, parse_image_inspect};
pub use login::parse_login_status;
pub use policy::{EXPECTED_NETWORK_POLICY, parse_network_policy};
pub use sandbox::{SandboxEntry, SandboxState, parse_sandbox_list};
pub use secret::{CustomSecret, parse_custom_secrets};
pub use template::{TemplateEntry, parse_template_list};
pub use version::{CliVersion, require_minimum_version};
