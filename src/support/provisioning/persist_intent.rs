use crate::command::HostEnvironment;
use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::metadata::{self, InitialProvisioningIntent, ProjectMetadata};
use crate::msg;
use crate::support::image;
use crate::support::select::Locked;

/// intentを最初の構築mutationとしてatomicに保存する。
///
/// 保存済みintentと異なるtargetを指定された場合、その世代のimageがまだ無ければ、
/// 通常のprepareと同じくretargetする。`docker build`の失敗などimmutable artifactが
/// 何も残らなかった段階まではDockerfileの再編集を続けられる。既にimageがある場合
/// だけ、それを上書きせずに拒否する。
pub(crate) fn persist_intent(
    host: &dyn HostEnvironment,
    locked: &mut Locked,
    target: &str,
) -> Result<()> {
    if let Some(intent) = &locked.metadata.initial_provisioning {
        if intent.target_dockerfile_sha256 == target {
            return Ok(());
        }
        if image::generation_is_built(
            host,
            &locked.metadata.sandbox_name(),
            locked.metadata.canonical_id(),
            &intent.target_dockerfile_sha256,
        )? {
            return Err(invalid_intent(&locked.metadata, target));
        }
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
