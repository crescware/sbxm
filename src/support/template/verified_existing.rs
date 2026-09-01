use std::path::Path;

use crate::archive;
use crate::boundary::host::HostEnvironment;
use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::hash::short_hex;
use crate::msg;

use crate::support::image::BuiltImage;

use super::{LoadedTemplate, find};

/// 名前が一致する既存Templateを、runtime idまで確認してから再利用する。
///
/// `sbx template ls --json`のrepositoryとtagだけでは、別内容のTemplateが同じ名前で
/// 登録されていても見分けられない。archiveのconfig digestを、label検証済みhost image
/// から作った期待値として使う。
pub fn verified_existing(
    host: &dyn HostEnvironment,
    image: &BuiltImage,
    archive_path: &Path,
) -> Result<Option<LoadedTemplate>> {
    let Some(entry) = find(host, &image.name)? else {
        return Ok(None);
    };
    let manifest = archive::read_manifest(archive_path)?;
    let expected = short_hex(
        manifest
            .config_digest
            .strip_prefix("sha256:")
            .unwrap_or(&manifest.config_digest),
    );

    match entry.id.as_deref() {
        Some(observed) if normalize(observed) == expected => Ok(Some(LoadedTemplate {
            name: image.name.clone(),
            loaded: false,
        })),
        Some(observed) => Err(mismatched(&image.name, observed, expected)),
        None => Err(unobservable_id(&image.name)),
    }
}

/// runtimeが返すidの表記幅を、比較できる形へ揃える。
fn normalize(id: &str) -> &str {
    short_hex(id.strip_prefix("sha256:").unwrap_or(id))
}

fn mismatched(name: &str, observed: &str, expected: &str) -> Error {
    Error::single(
        Diagnostic::new(ErrorId::TemplateUnusable, msg!("error-template-unusable"))
            .fact(Fact::template(name))
            .fact(Fact::reason(msg!(
                "cause-template-id-differs",
                observed = observed,
                expected = expected
            ))),
    )
}

fn unobservable_id(name: &str) -> Error {
    Error::single(
        Diagnostic::new(ErrorId::TemplateUnusable, msg!("error-template-unusable"))
            .fact(Fact::template(name))
            .fact(Fact::reason(msg!("cause-template-id-absent"))),
    )
}

#[cfg(test)]
#[path = "verified_existing_test.rs"]
mod verified_existing_test;
