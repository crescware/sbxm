use crate::design::{Fact, Inline, Remediation};
use crate::diagnostics::{Diagnostic, Error, ErrorId};
use crate::hash::short_hex;
use crate::metadata::ProjectMetadata;
use crate::msg;

use super::ProvisioningState;

/// prepareが暗黙に継続せず、明示的なrepairへ渡す診断を作る。
pub(crate) fn require_repair(metadata: &ProjectMetadata, state: ProvisioningState) -> Error {
    let (id, description) = match state {
        ProvisioningState::Pending => (
            ErrorId::InitialProvisioningPending,
            msg!(
                "error-initial-provisioning-pending",
                project = metadata.display_id()
            ),
        ),
        ProvisioningState::Incomplete
        | ProvisioningState::Fresh
        | ProvisioningState::Ready
        | ProvisioningState::Unobservable => (
            ErrorId::InitialProvisioningIncomplete,
            msg!(
                "error-initial-provisioning-incomplete",
                project = metadata.display_id()
            ),
        ),
    };
    let mut diagnostic =
        Diagnostic::new(id, description).fact(Fact::sandbox(&metadata.sandbox_name().to_string()));
    if state == ProvisioningState::Pending
        && let Some(intent) = &metadata.initial_provisioning
    {
        diagnostic = diagnostic.fact(Fact::new(
            msg!("diagnostic-fixed-target-generation-label"),
            Inline::important(short_hex(&intent.target_dockerfile_sha256)),
        ));
    }
    Error::single(
        diagnostic
            .fact(Fact::reason(msg!(
                "cause-provisioning-state",
                state = state
            )))
            .remediation(
                Remediation::text(msg!("remediation-run-repair"))
                    .try_run(format!("sbxm repair {}", metadata.display_id())),
            ),
    )
}
