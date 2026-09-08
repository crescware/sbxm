use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::metadata::InitialProvisioningIntent;
use crate::msg;

use super::{ProvisioningInputs, initial_intent};

/// 取得済みsnapshotが、初回構築時に固定したintentそのものであることを確認する。
///
/// 生きているsourceを検証してから改めてsnapshotへ読むと、その間の書き換えを許す。
/// repairの実行直前は先にsnapshotを固定し、そのbyte列から作ったintentと比較する。
pub(crate) fn validate_captured_intent(
    intent: &InitialProvisioningIntent,
    inputs: &ProvisioningInputs,
    project: &str,
) -> Result<()> {
    let captured = initial_intent(inputs);
    if intent.target_dockerfile_sha256 != captured.target_dockerfile_sha256 {
        return Err(changed(project, "target"));
    }
    if intent.files.len() != captured.files.len() {
        return Err(changed(project, "files"));
    }
    for (index, (expected, observed)) in intent.files.iter().zip(&captured.files).enumerate() {
        if expected != observed {
            return Err(changed(project, &index.to_string()));
        }
    }
    Ok(())
}

fn changed(project: &str, entry: &str) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::InitialProvisioningInputChanged,
            msg!(
                "error-initial-provisioning-input-changed",
                project = project
            ),
        )
        .fact(Fact::entry(entry))
        .fact(Fact::reason(msg!(
            "cause-initial-provisioning-input-changed"
        )))
        .remediation(msg!("remediation-initial-provisioning-input-changed")),
    )
}

#[cfg(test)]
#[path = "validate_captured_intent_test.rs"]
mod validate_captured_intent_test;
