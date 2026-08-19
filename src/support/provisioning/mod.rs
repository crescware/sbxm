//! 初回構築を共有するworkflow境界。
//!
//! `prepare`と`repair`は入口の契約だけを持ち、Sandbox、image、Template、repositoryを
//! 進める処理はここだけを使う。初回構築のintentを保持したまま、どの工程からでも同じ
//! allowlistの検査と再利用規則へ戻れるようにする。

mod already_built;
mod changed_dockerfile_warning;
mod clear_intent;
mod external_preconditions;
mod fresh_target;
mod observed_worktrees;
mod persist_intent;
mod provision;
mod provisioning_output;
mod verify_external_preconditions;
mod worktree_row;

pub(crate) use already_built::already_built;
pub(crate) use changed_dockerfile_warning::changed_dockerfile_warning;
pub(crate) use clear_intent::clear_intent;
pub(crate) use external_preconditions::ExternalPreconditions;
pub(crate) use fresh_target::fresh_target;
pub(crate) use observed_worktrees::observed_worktrees;
pub(crate) use persist_intent::persist_intent;
pub(crate) use provision::provision;
pub use provisioning_output::ProvisioningOutput;
pub(crate) use verify_external_preconditions::verify_external_preconditions;
pub use worktree_row::WorktreeRow;

#[cfg(test)]
#[path = "provisioning_test.rs"]
mod provisioning_test;
