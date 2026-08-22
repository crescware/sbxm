use crate::command::HostEnvironment;
use crate::compatibility::ImageIdentity;
use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;
use crate::project::{CanonicalProjectId, SandboxName};

use super::{
    VerifiedGeneration, compare_labels, expected_labels, image_name, inspect, labels_match,
};

/// 既存imageが指定された案件・世代のものかをread-onlyで確認する。
///
/// 不在は、後続が作成できる状態として成功にする。同名imageのlabelが異なる場合は、
/// 何も変更しないまま停止する。観測した同一性は証跡として返し、呼び出し側が同じ
/// imageをinspectし直さずに済むようにする。
pub(crate) fn verify_generation(
    host: &dyn HostEnvironment,
    sandbox: &SandboxName,
    canonical: &CanonicalProjectId,
    dockerfile_sha256: &str,
) -> Result<VerifiedGeneration> {
    let name = image_name(sandbox, dockerfile_sha256);
    let Some(identity) = inspect(host, &name)? else {
        return Ok(VerifiedGeneration(None));
    };
    let expected = expected_labels(canonical, dockerfile_sha256);
    if labels_match(&identity, &expected) {
        return Ok(VerifiedGeneration(Some(identity)));
    }
    Err(collision(&name, &identity, &expected))
}

/// 同じ世代名を持つ、別の案件または別の世代のimage。
///
/// 名前だけで同一とみなして上書きすると、利用者の成果物を失う。
fn collision(name: &str, identity: &ImageIdentity, expected: &[(String, String)]) -> Error {
    Error::single(
        Diagnostic::new(ErrorId::ImageUnusable, msg!("error-image-collision"))
            .fact(Fact::image(name))
            .fact(Fact::cause(&compare_labels(identity, expected)))
            .remediation(msg!("remediation-image-collision", image = name)),
    )
}
