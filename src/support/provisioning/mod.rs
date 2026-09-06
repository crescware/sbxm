//! 初回構築を共有するworkflow境界。
//!
//! 入口となるcommandは対象の解決と表示だけを持ち、Sandbox、image、Template、
//! repositoryを進める処理はここだけを使う。どの入口から入っても、同じ検査と
//! 同じ再利用規則を通る。

mod build_initial;
mod declared_files;
mod ensure_initial;
mod external_preconditions;
mod initial_intent;
mod initial_route;
mod next_action;
mod observation;
mod observe;
mod observed_worktrees;
mod provision;
mod provisioning_inputs;
mod provisioning_output;
mod provisioning_state;
mod ready_output;
mod require_no_initial_intent;
mod require_observable;
mod require_repair;
mod snapshot_file;
mod validate_intent;
mod verify_external_preconditions;
mod worktree_row;

pub(crate) use build_initial::build_initial;
pub(crate) use ensure_initial::ensure_initial;
pub(crate) use external_preconditions::ExternalPreconditions;
pub(crate) use initial_intent::initial_intent;
pub(crate) use initial_route::InitialRoute;
pub use next_action::NextAction;
pub use observation::Observation;
pub(crate) use observe::observe;
pub(crate) use provision::provision;
pub(crate) use provisioning_inputs::ProvisioningInputs;
pub use provisioning_output::ProvisioningOutput;
pub use provisioning_state::ProvisioningState;
pub(crate) use ready_output::ready_output;
pub(crate) use require_no_initial_intent::require_no_initial_intent;
pub(crate) use require_observable::require_observable;
pub(crate) use require_repair::require_repair;
pub(crate) use snapshot_file::SnapshotFile;
pub(crate) use validate_intent::validate_intent;
pub(crate) use verify_external_preconditions::verify_external_preconditions;
pub use worktree_row::WorktreeRow;

#[cfg(test)]
#[path = "provisioning_test.rs"]
mod provisioning_test;
