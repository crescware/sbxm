//! 外部 command の出力を、protocol 固有の表現から内部で扱う値へ変換する。
//!
//! `boundary::host` が process を実行して stdout / stderr を運ぶ。ここではその出力を
//! parse し、解釈できないものは状態として推測せず error にする。
//!
//! protocol ごとに parser を分け、複数の parser に共通する structured output の処理だけを
//! `json` に置く。

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
