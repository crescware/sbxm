use std::path::Path;

use crate::command::HostEnvironment;
use crate::compatibility::ImageIdentity;
use crate::design::{Fact, ProgressSink};
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;
use crate::project::{CanonicalProjectId, SandboxName};

use super::{BuiltImage, build, expected_labels, image_name, inspect, labels_match};

/// 世代に対応するimageを用意する。
///
/// 既存imageは、全labelが一致した場合だけ再利用する。
pub fn ensure(
    host: &dyn HostEnvironment,
    sandbox: &SandboxName,
    canonical: &CanonicalProjectId,
    dockerfile: &Path,
    dockerfile_sha256: &str,
    progress: &mut dyn ProgressSink,
) -> Result<BuiltImage> {
    let name = image_name(sandbox, dockerfile_sha256);
    let labels = expected_labels(canonical, dockerfile_sha256);

    if let Some(identity) = inspect(host, &name)? {
        if !labels_match(&identity, &labels) {
            // 世代名が同じでも中身は別物である。この名前の既存成果物を作り直さない。
            return Err(collision(&name, &identity, &labels));
        }
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
        Error::new(
            ErrorId::ImageUnusable,
            msg!(
                "error-image-unusable",
                image = name,
                detail = "the image is absent right after it was built"
            ),
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
    Error::single(Diagnostic::new(
        ErrorId::ImageUnusable,
        msg!(
            "error-image-unusable",
            image = name,
            detail = compare_labels(identity, expected)
        ),
    ))
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

/// 期待するlabelと観測したlabelの並び。翻訳しない技術表記。
fn compare_labels(identity: &ImageIdentity, expected: &[(String, String)]) -> String {
    expected
        .iter()
        .map(|(key, value)| {
            let observed = identity.labels.get(key).map_or("<absent>", String::as_str);
            format!("{key}: expected {value}, observed {observed}")
        })
        .collect::<Vec<_>>()
        .join("; ")
}
