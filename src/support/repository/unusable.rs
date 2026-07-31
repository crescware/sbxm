use crate::diagnostics::{Diagnostic, Error, ErrorId};
use crate::msg;

/// 成果物を自動削除せず、観測した不一致を示して停止する。
pub(super) fn unusable(path: &str, detail: &str) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::SandboxRepositoryUnusable,
            msg!(
                "error-sandbox-repository-unusable",
                path = path,
                detail = detail
            ),
        )
        .remediation(msg!("remediation-sandbox-repository-unusable", path = path)),
    )
}
