use crate::command::HostEnvironment;
use crate::compatibility::ImageIdentity;
use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;
use crate::project::{CanonicalProjectId, SandboxName};

use super::{expected_labels, image_name, inspect, labels_match};

/// 既存imageが指定された案件・世代のものかをread-onlyで確認する。
///
/// 不在は、後続のprovisioningが作成できる状態として成功にする。同名imageのlabelが
/// 異なる場合は、repairの計画を返す前に停止し、intentやほかの成果物を変更しない。
pub(crate) fn verify_generation(
    host: &dyn HostEnvironment,
    sandbox: &SandboxName,
    canonical: &CanonicalProjectId,
    dockerfile_sha256: &str,
) -> Result<()> {
    let name = image_name(sandbox, dockerfile_sha256);
    let Some(identity) = inspect(host, &name)? else {
        return Ok(());
    };
    let expected = expected_labels(canonical, dockerfile_sha256);
    if labels_match(&identity, &expected) {
        return Ok(());
    }
    Err(collision(&name, &identity, &expected))
}

fn collision(name: &str, identity: &ImageIdentity, expected: &[(String, String)]) -> Error {
    Error::single(
        Diagnostic::new(ErrorId::ImageUnusable, msg!("error-image-collision"))
            .fact(Fact::image(name))
            .fact(Fact::cause(&compare_labels(identity, expected)))
            .remediation(msg!("remediation-image-collision", image = name)),
    )
}

fn compare_labels(identity: &ImageIdentity, expected: &[(String, String)]) -> String {
    expected
        .iter()
        .map(|(key, value)| {
            let observed = identity.labels.get(key).map_or("<absent>", String::as_str);
            format!("{key}: expected {value}, observed {observed}")
        })
        .collect::<Vec<_>>()
        .join("\n")
}
