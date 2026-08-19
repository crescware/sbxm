use std::path::Path;

use crate::command::HostEnvironment;
use crate::compatibility::ImageIdentity;
use crate::design::{Fact, ProgressSink};
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;
use crate::project::{CanonicalProjectId, SandboxName};

use super::{
    BuiltImage, VerifiedGeneration, build, compare_labels, expected_labels, image_name, inspect,
    labels_match,
};

/// 衝突が無いと確認済みの世代へ、imageを用意する。
///
/// 確認時の観測をそのまま使う。再利用できるかどうかを決めるためだけに、同じimageを
/// もう一度inspectしない。
#[allow(clippy::too_many_arguments)]
pub(crate) fn ensure_verified(
    host: &dyn HostEnvironment,
    sandbox: &SandboxName,
    canonical: &CanonicalProjectId,
    dockerfile: &Path,
    dockerfile_sha256: &str,
    verified: VerifiedGeneration,
    progress: &mut dyn ProgressSink,
) -> Result<BuiltImage> {
    let name = image_name(sandbox, dockerfile_sha256);
    let labels = expected_labels(canonical, dockerfile_sha256);

    if let Some(identity) = verified.0 {
        return Ok(BuiltImage {
            name,
            id: identity.id,
            labels,
            built: false,
            warnings: Vec::new(),
        });
    }

    let warnings = build(host, &name, &labels, dockerfile, progress)?;

    let identity = inspect(host, &name)?.ok_or_else(|| {
        Error::single(
            Diagnostic::new(ErrorId::ImageUnusable, msg!("error-image-unusable"))
                .fact(Fact::image(&name))
                .fact(Fact::reason(msg!("cause-image-absent-after-build"))),
        )
    })?;
    if !labels_match(&identity, &labels) {
        return Err(mismatched_labels(&name, &identity, &labels));
    }

    Ok(BuiltImage {
        name,
        id: identity.id,
        labels,
        built: true,
        warnings,
    })
}

fn mismatched_labels(name: &str, identity: &ImageIdentity, expected: &[(String, String)]) -> Error {
    Error::single(
        Diagnostic::new(ErrorId::ImageUnusable, msg!("error-image-unusable"))
            .fact(Fact::image(name))
            .fact(Fact::cause(&compare_labels(identity, expected))),
    )
}
