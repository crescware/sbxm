use crate::boundary::host::CommandOutcome;
use crate::design::{Fact, Remediation};
use crate::diagnostics::{Diagnostic, Error, ErrorId};
use crate::msg;
use crate::support::sandbox::{neutralized, ssh_host};

/// hostのgitが、`sandbox`のrepositoryを読めなかったこと。
///
/// 原因はgitの答えのまま示す。gitが中継したSandboxの出力を含むため、制御文字は見える
/// 形にする。sshでつながらないことが多いため、つながるかを確かめるcommandを添える。
/// 起動したgitのcommand lineは示さない。長いだけで、原因を読む助けにならない。
pub fn sandbox_unreadable(sandbox: &str, outcome: &CommandOutcome) -> Error {
    let stderr = String::from_utf8_lossy(&outcome.stderr);
    Error::single(
        Diagnostic::new(
            ErrorId::SandboxRepositoryUnreadable,
            msg!("error-sandbox-repository-unreadable", sandbox = sandbox),
        )
        .fact(Fact::cause(&neutralized(stderr.trim())))
        .remediation(
            Remediation::text(msg!("remediation-sandbox-repository-over-ssh"))
                .try_run(format!("ssh {} true", ssh_host(sandbox))),
        ),
    )
}
