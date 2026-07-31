use crate::design::Remediation;
use crate::diagnostics::{Diagnostic, Error, ErrorId};
use crate::msg;
use crate::paths::{self};

use super::Candidate;

/// registryへは登録されているが、project metadataがまだ無い。
///
/// 登録意図は残っているため、同じ要求の`add`だけが続きを実行できる。
pub fn incomplete_registration(candidate: &Candidate) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::ProjectIncomplete,
            msg!(
                "error-project-incomplete",
                project = candidate.display_id(),
                path = paths::display(candidate.paths.root())
            ),
        )
        .remediation(
            Remediation::text(msg!("remediation-project-incomplete"))
                .try_run(format!("sbxm add {}", candidate.repository.clone_url())),
        ),
    )
}
