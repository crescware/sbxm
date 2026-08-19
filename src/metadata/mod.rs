//! Project metadata `<project-root>/.sbxm/project.yaml`。
//!
//! metadataは進捗cacheではなく、利用者が要求した目標構成である。sbxmは成果物の
//! 作成元を追跡しないため、validation規則を満たすmetadataは、誰が書いたかを問わず
//! 同じものとして扱う。

mod create;
mod creation_mode;
mod document;
mod document_version;
mod git_identity;
mod last_worktree_index;
mod load;
mod max_worktree_index;
mod max_worktrees;
mod metadata_version;
mod min_worktrees;
mod missing;
#[cfg(test)]
#[path = "mod_test.rs"]
mod mod_test;
mod parse;
mod project_metadata;
mod provisioning;
mod rebuild_intent;
mod render;
mod require_sha256;
mod update;
mod validate_git_identity_value;

pub use create::create;
pub use creation_mode::CreationMode;
use document_version::DOCUMENT_VERSION;
pub use git_identity::GitIdentity;
pub use last_worktree_index::last_worktree_index;
pub use load::load;
pub use max_worktree_index::MAX_WORKTREE_INDEX;
pub use max_worktrees::MAX_WORKTREES;
pub use metadata_version::METADATA_VERSION;
pub use min_worktrees::MIN_WORKTREES;
use missing::missing;
pub use parse::parse;
pub use project_metadata::ProjectMetadata;
pub use provisioning::Provisioning;
pub use rebuild_intent::RebuildIntent;
pub use render::render;
use require_sha256::require_sha256;
pub use update::update;
pub use validate_git_identity_value::validate_git_identity_value;
