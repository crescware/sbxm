//! 初回構築を共有するworkflow境界。
//!
//! 入口となるcommandは対象の解決と表示だけを持ち、Sandbox、image、Template、
//! repositoryを進める処理はここだけを使う。どの入口から入っても、同じ検査と
//! 同じ再利用規則を通る。

mod already_built;
mod changed_dockerfile_warning;
mod external_preconditions;
mod fresh_target;
mod observed_worktrees;
mod provision;
mod provisioning_output;
mod verify_external_preconditions;
mod worktree_row;

pub(crate) use already_built::already_built;
pub(crate) use external_preconditions::ExternalPreconditions;
pub(crate) use fresh_target::fresh_target;
pub(crate) use provision::provision;
pub use provisioning_output::ProvisioningOutput;
pub(crate) use verify_external_preconditions::verify_external_preconditions;
pub use worktree_row::WorktreeRow;

#[cfg(test)]
#[path = "provisioning_test.rs"]
mod provisioning_test;
