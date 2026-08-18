//! 初回構築を共有するworkflow境界。
//!
//! `prepare`と`repair`は入口の契約だけを持ち、Sandbox、image、Template、repositoryを
//! 進める処理はここだけを使う。初回構築のintentを保持したまま、どの工程からでも同じ
//! allowlistの検査と再利用規則へ戻れるようにする。

mod already_built;
mod artifact;
mod changed_dockerfile_warning;
mod clear_intent;
mod fresh_target;
mod observation;
mod observe;
mod observed_worktrees;
mod persist_intent;
mod preflight;
mod provision;
mod provisioning_output;
mod provisioning_state;
mod worktree_row;

pub(crate) use already_built::already_built;
pub use artifact::Artifact;
pub(crate) use changed_dockerfile_warning::changed_dockerfile_warning;
pub(crate) use clear_intent::clear_intent;
pub(crate) use fresh_target::fresh_target;
pub use observation::Observation;
pub use observe::observe;
pub(crate) use observed_worktrees::observed_worktrees;
pub(crate) use persist_intent::persist_intent;
pub(crate) use preflight::preflight;
pub(crate) use provision::provision;
pub use provisioning_output::ProvisioningOutput;
pub use provisioning_state::ProvisioningState;
pub use worktree_row::WorktreeRow;

#[cfg(test)]
#[path = "provisioning_test.rs"]
mod provisioning_test;
