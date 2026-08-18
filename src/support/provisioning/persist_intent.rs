use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::metadata::{self, InitialProvisioningIntent, ProjectMetadata};
use crate::msg;
use crate::support::select::Locked;

/// intentを最初の構築mutationとしてatomicに保存する。
pub(crate) fn persist_intent(locked: &mut Locked, target: &str) -> Result<()> {
    if let Some(intent) = &locked.metadata.initial_provisioning {
        if intent.target_dockerfile_sha256 != target {
            return Err(invalid_intent(&locked.metadata, target));
        }
        return Ok(());
    }

    let mut metadata = locked.metadata.clone();
    metadata.provisioning.dockerfile_sha256 = target.to_string();
    metadata.initial_provisioning = Some(InitialProvisioningIntent {
        target_dockerfile_sha256: target.to_string(),
    });
    metadata::update(&locked.paths, &metadata)?;
    locked.metadata = metadata;
    Ok(())
}

fn invalid_intent(metadata: &ProjectMetadata, target: &str) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::InitialProvisioningInvalid,
            msg!(
                "error-initial-provisioning-invalid",
                project = metadata.display_id()
            ),
        )
        .fact(Fact::value(target))
        .remediation(msg!("remediation-initial-provisioning-invalid")),
    )
}
