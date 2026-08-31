use crate::design::{Fact, Remediation};
use crate::diagnostics::{Diagnostic, Error, ErrorId};
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
        ProvisioningState::Incomplete | ProvisioningState::Fresh | ProvisioningState::Ready => (
            ErrorId::InitialProvisioningIncomplete,
            msg!(
                "error-initial-provisioning-incomplete",
                project = metadata.display_id()
            ),
        ),
    };
    Error::single(
        Diagnostic::new(id, description)
            .fact(Fact::sandbox(&metadata.sandbox_name().to_string()))
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
