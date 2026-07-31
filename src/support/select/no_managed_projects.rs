use crate::design::Remediation;
use crate::diagnostics::{Diagnostic, Error, ErrorId};
use crate::msg;
use crate::repository::CLONE_URL_PLACEHOLDER;

/// 選択候補となる管理案件が0件であることを、対象選択を開始できないerrorとして返す。
pub(super) fn no_managed_projects() -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::NoManagedProjects,
            msg!("error-no-managed-projects"),
        )
        .remediation(
            Remediation::text(msg!("remediation-no-managed-projects"))
                .try_run(format!("sbxm add {CLONE_URL_PLACEHOLDER}")),
        ),
    )
}
