//! `boundary::host` が収集した標準出力を、後続の処理が扱う内部の値へ変換する。
//!
//! process の起動と標準出力の収集は `boundary::host` が担う。この module は収集した
//! 文字列の形式を検証し、期待した形式でなければ状態を推測せず error を返す。
//!
//! 出力形式ごとの解釈を module に分け、複数の解釈処理で共通する JSON の読み取りだけを
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
