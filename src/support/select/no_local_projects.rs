use crate::design::Remediation;
use crate::diagnostics::{Diagnostic, Error, ErrorId};
use crate::msg;

/// `--local`で追加した案件が1つも無いことを、対象選択を開始できないerrorとして返す。
pub fn no_local_projects() -> Error {
    Error::single(
        Diagnostic::new(ErrorId::NoLocalProjects, msg!("error-no-local-projects")).remediation(
            Remediation::text(msg!("remediation-no-local-projects"))
                .try_run("sbxm add --local <git-dir>"),
        ),
    )
}
