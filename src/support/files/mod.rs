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
mod divergence;
mod max_source_bytes;
mod observe;
mod observe_against_baseline;
mod place_all;
mod place_from_stdin;
mod placed_file;
mod placement;
mod plan;
mod plan_all;
mod planned_file;
mod read_source;
mod read_source_bytes;
mod receive_copy;
mod received_copy;
mod require_no_symlink_in_sandbox;
mod sandbox_digest;
mod transfer_incomplete;

use agent_home::AGENT_HOME;
pub use conflict::Conflict;
use copy_into_sandbox::copy_into_sandbox;
use copy_steps::copy_steps;
use destination_path::destination_path;
use digest_in_sandbox::digest_in_sandbox;
pub use divergence::Divergence;
pub use max_source_bytes::MAX_SOURCE_BYTES;
pub use observe::observe;
pub use observe_against_baseline::observe_against_baseline;
pub use place_all::place_all;
use place_from_stdin::PLACE_FROM_STDIN;
pub use placed_file::PlacedFile;
pub use placement::Placement;
use plan::plan;
pub use plan_all::plan_all;
pub use planned_file::PlannedFile;
pub use read_source::read_source;
pub use read_source_bytes::read_source_bytes;
pub use receive_copy::receive_copy;
pub use received_copy::ReceivedCopy;
use require_no_symlink_in_sandbox::require_no_symlink_in_sandbox;
pub use sandbox_digest::sandbox_digest;
use transfer_incomplete::TRANSFER_INCOMPLETE;

#[cfg(test)]
#[path = "files_test.rs"]
mod files_test;
