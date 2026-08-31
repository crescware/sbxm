//! Global configが宣言したfileのSandboxへの配置。
//!
//! 特定のAgentやtoolの設定形式を解釈せず、利用者が宣言したfileだけを、宣言された
//! 相対pathへ置く。file内容はstdout、stderr、log、metadataへ出さない。

mod agent_home;
mod conflict;
mod copy_into_sandbox;
mod copy_steps;
mod destination_path;
mod digest_in_sandbox;
mod max_source_bytes;
mod observe;
mod place;
mod place_all;
mod placed_file;
mod placement;
mod read_source;
mod require_no_symlink_in_sandbox;

use agent_home::AGENT_HOME;
pub use conflict::Conflict;
use copy_into_sandbox::copy_into_sandbox;
use copy_steps::copy_steps;
use destination_path::destination_path;
use digest_in_sandbox::digest_in_sandbox;
use max_source_bytes::MAX_SOURCE_BYTES;
pub use observe::observe;
use place::place;
pub use place_all::place_all;
pub use placed_file::PlacedFile;
pub use placement::Placement;
pub use read_source::read_source;
use require_no_symlink_in_sandbox::require_no_symlink_in_sandbox;

#[cfg(test)]
#[path = "files_test.rs"]
mod files_test;
