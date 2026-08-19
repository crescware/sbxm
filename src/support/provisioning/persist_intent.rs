use crate::command::HostEnvironment;
use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::metadata::{self, InitialProvisioningIntent, ProjectMetadata};
use crate::msg;
use crate::support::image;
use crate::support::select::Locked;

use super::ObservedGeneration;

/// intentを最初の構築mutationとしてatomicに保存する。
///
/// 保存済みintentと異なるtargetを指定された場合、その世代のimageがまだ無ければ、
/// 通常のprepareと同じくretargetする。`docker build`の失敗などimmutable artifactが
/// 何も残らなかった段階まではDockerfileの再編集を続けられる。既にimageがある場合
/// だけ、それを上書きせずに拒否する。
///
/// 放棄されるtargetを呼び出し側が既に観測している場合はその結果を使い、同じimageを
/// 観測し直さない。
pub(crate) fn persist_intent(
    host: &dyn HostEnvironment,
    locked: &mut Locked,
    target: &str,
    observed: Option<&ObservedGeneration>,
) -> Result<()> {
    if let Some(intent) = &locked.metadata.initial_provisioning {
        let abandoned = intent.target_dockerfile_sha256.clone();
        if abandoned == target {
            return Ok(());
        }
        if abandoned_target_is_built(host, locked, &abandoned, observed)? {
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

/// 放棄されるtargetのimageが既にあるか。観測済みの世代が一致すればそれを信じる。
fn abandoned_target_is_built(
    host: &dyn HostEnvironment,
    locked: &Locked,
    abandoned: &str,
    observed: Option<&ObservedGeneration>,
) -> Result<bool> {
    if let Some(observed) = observed
        && observed.dockerfile_sha256 == abandoned
    {
        return Ok(observed.built);
    }
    image::generation_is_built(
        host,
        &locked.metadata.sandbox_name(),
        locked.metadata.canonical_id(),
        abandoned,
    )
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
