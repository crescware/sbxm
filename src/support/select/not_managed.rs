use crate::design::Remediation;
use crate::diagnostics::{Diagnostic, Error, ErrorId};
use crate::msg;
use crate::repository::CLONE_URL_PLACEHOLDER;

/// 管理対象でない案件を、登録commandとともに拒否する。
///
/// 登録にはclone URLが要る。未登録の案件からはtransportを決められないため、
/// URLそのものは推測せず、利用者が差し替える位置を示す。
pub fn not_managed(project: &dyn std::fmt::Display) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::ProjectNotManaged,
            msg!("error-project-not-managed", project = project),
        )
        .remediation(
            Remediation::text(msg!("remediation-project-not-managed"))
                .try_run(format!("sbxm add {CLONE_URL_PLACEHOLDER}")),
        ),
    )
}
