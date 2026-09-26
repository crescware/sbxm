use crate::design::{Fact, Remediation};
use crate::diagnostics::{Diagnostic, Error, ErrorId, Msg};
use crate::msg;

/// hostにあるrepositoryの案件で、Sandboxのoriginが宣言と違うことを示して停止する。
///
/// hostは今、Sandboxのoriginをssh越しに書き込む。以前のsbxmが作ったSandboxは、originを
/// 別の場所へ向けている。成果物を自動削除しないことは`unusable`と同じであり、作り直しを
/// 案内する。作り直しは、何かを消す前に、Sandboxのcommitがhostにあることを確かめる。
pub(super) fn unusable_host_origin(path: &str, reason: Msg, project: &str) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::SandboxRepositoryUnusable,
            msg!("error-sandbox-repository-unusable"),
        )
        .fact(Fact::path(path))
        .fact(Fact::reason(reason))
        .remediation(
            Remediation::text(msg!("remediation-sandbox-host-origin-differs"))
                .try_run(format!("sbxm rebuild {project}")),
        ),
    )
}
