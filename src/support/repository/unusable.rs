use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Msg};
use crate::msg;

/// 成果物を自動削除せず、観測した不一致を示して停止する。
pub(super) fn unusable(path: &str, reason: Msg) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::SandboxRepositoryUnusable,
            msg!("error-sandbox-repository-unusable"),
        )
        .fact(Fact::path(path))
        .fact(Fact::reason(reason))
        .remediation(msg!("remediation-sandbox-repository-unusable", path = path)),
    )
}
