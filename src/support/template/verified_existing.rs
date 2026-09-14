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
/// 登録されていても見分けられない。label検証済みhost imageから作ったarchiveが宣言する
/// digestを期待値として使う。
///
/// runtimeが報告するidは、archiveがOCI layoutならindexまたはmanifestのdigest、
/// legacy layoutならimage configのdigestになる。どちらか一方に決め打ちすると、
/// 同じimageから作った正しいTemplateを別物として拒み、2度目の準備が進まなくなる。
/// archiveが宣言する候補のいずれかと一致すれば、同じimageと判定する。
pub fn verified_existing(
    host: &dyn HostEnvironment,
    image: &BuiltImage,
    archive_path: &Path,
) -> Result<Option<LoadedTemplate>> {
    let Some(entry) = find(host, &image.name)? else {
        return Ok(None);
    };
    let declared = archive::read_image_ids(archive_path)?;

    match entry.id.as_deref() {
        Some(observed)
            if declared
                .iter()
                .any(|id| normalize(id) == normalize(observed)) =>
        {
            Ok(Some(LoadedTemplate {
                name: image.name.clone(),
                loaded: false,
            }))
        }
        Some(observed) => Err(mismatched(&image.name, observed, &declared)),
        None => Err(unobservable_id(&image.name)),
    }
}

/// runtimeが返すidの表記幅を、比較できる形へ揃える。
fn normalize(id: &str) -> &str {
    short_hex(id.strip_prefix("sha256:").unwrap_or(id))
}

fn mismatched(name: &str, observed: &str, declared: &[String]) -> Error {
    let expected = declared
        .iter()
        .map(|id| normalize(id))
        .collect::<Vec<_>>()
        .join(", ");
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
