//! 初回構築を共有するworkflow境界。
//!
//! 入口となるcommandは対象の解決と表示だけを持ち、Sandbox、image、Template、
//! repositoryを進める処理はここだけを使う。どの入口から入っても、同じ検査と
//! 同じ再利用規則を通る。

mod already_built;
mod artifact;
mod changed_dockerfile_warning;
mod clear_intent;
mod external_preconditions;
mod fresh_target;
mod observation;
mod observe;
mod observed_worktrees;
mod persist_intent;
mod preflight;
mod provision;
mod provisioning_output;
mod provisioning_state;
mod verify_external_preconditions;
mod worktree_row;

pub(crate) use already_built::already_built;
pub use artifact::Artifact;
pub(crate) use changed_dockerfile_warning::changed_dockerfile_warning;
pub(crate) use clear_intent::clear_intent;
pub(crate) use external_preconditions::ExternalPreconditions;
pub(crate) use fresh_target::fresh_target;
pub use observation::Observation;
pub use observe::observe;
pub(crate) use observed_worktrees::observed_worktrees;
pub(crate) use persist_intent::persist_intent;
pub(crate) use preflight::preflight;
pub(crate) use provision::provision;
pub use provisioning_output::ProvisioningOutput;
pub use provisioning_state::ProvisioningState;
pub(crate) use verify_external_preconditions::verify_external_preconditions;
pub use worktree_row::WorktreeRow;

#[cfg(test)]
#[path = "provisioning_test.rs"]
mod provisioning_test;
