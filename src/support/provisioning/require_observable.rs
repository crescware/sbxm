use crate::design::{Fact, Remediation};
use crate::diagnostics::{Diagnostic, Error, ErrorId};
use crate::metadata::ProjectMetadata;
use crate::msg;

use super::Observation;

/// 完成しているかどうかを、変更なしには観測できない案件を拒否する診断。
///
/// 停止中のSandboxの中は、読むcommand自体がSandboxを起動し得る。read-onlyのはずの
/// 判定でhostを動かさないため、ここでは起動せずに拒否する。欠落と決めてrepairへ
/// 送ることも、完成と決めて進むこともしない。
pub(crate) fn require_observable(metadata: &ProjectMetadata, observation: &Observation) -> Error {
    let state = observation
        .sandbox_state
        .map_or("unknown", |state| state.as_str());
    Error::single(
        Diagnostic::new(
            ErrorId::InitialProvisioningUnobservable,
            msg!(
                "error-initial-provisioning-unobservable",
                project = metadata.display_id()
            ),
        )
        .fact(Fact::sandbox(&metadata.sandbox_name().to_string()))
        .fact(Fact::value(state))
        .fact(Fact::reason(msg!(
            "cause-initial-provisioning-unobservable",
            state = state
        )))
        .remediation(
            Remediation::text(msg!("remediation-initial-provisioning-unobservable"))
                .try_run(format!("sbxm open {}", metadata.display_id())),
        ),
    )
}

#[cfg(test)]
#[path = "require_observable_test.rs"]
mod require_observable_test;
