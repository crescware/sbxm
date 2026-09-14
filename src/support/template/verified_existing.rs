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
/// digest（`declared`、[`crate::archive::read_image_ids`]の結果）を期待値として使う。
///
/// runtimeが報告するidは、archiveがOCI layoutならindexまたはmanifestのdigestであり、
/// image configのdigestではない。configのdigestに決め打ちすると、同じimageから作った
/// 正しいTemplateを別物として拒み、2度目の準備が進まなくなる。archiveが宣言する候補の
/// いずれかと一致すれば同じimageと判定し、どれとも一致しなければ再利用しない。
pub fn verified_existing(
    host: &dyn HostEnvironment,
    image: &BuiltImage,
    declared: &[String],
) -> Result<Option<LoadedTemplate>> {
    let Some(entry) = find(host, &image.name)? else {
        return Ok(None);
    };
    let Some(observed) = entry.id.as_deref() else {
        return Err(unobservable_id(&image.name));
    };

    let expected: Vec<&str> = declared.iter().map(|id| normalize(id)).collect();
    if expected.contains(&normalize(observed)) {
        return Ok(Some(LoadedTemplate {
            name: image.name.clone(),
            loaded: false,
        }));
    }
    Err(mismatched(&image.name, observed, &expected.join(", ")))
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
