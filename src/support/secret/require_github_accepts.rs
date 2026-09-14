use crate::boundary::host::HostEnvironment;
use crate::design::{Fact, Remediation};
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::git;
use crate::msg;
use crate::project::ProjectId;

use crate::support::sandbox;

use super::{GITHUB_SERVICE, forget_command, register_command};

/// GitHubが認証を拒んだときに、gitがstderrへ書く文。
///
/// tokenが無効でも、sentinelが差し替えられずそのまま届いても、GitHubは同じ文で拒む。
/// 到達できない・repositoryが無いなど、認証以外の失敗はこの文を含まない。
const REJECTED: [&str; 3] = [
    "Authentication failed",
    "Invalid username or token",
    "could not read Username",
];

/// Sandboxの中のgitが、このrepositoryへ実際に認証できることを確かめる。
///
/// 登録の有無と、Sandboxが環境変数を持つことは、proxyが本物のtokenを差し替えることの
/// 証明にならない。tokenの期限切れや権限不足、以前の版のcustom secretとの衝突は、
/// どれもここで初めて現れる。長いfetchへ進む前に、実物と同じ経路で1回だけ問い合わせ、
/// 拒まれたときは何を登録し直せばよいかを示す。
pub fn require_github_accepts(
    host: &dyn HostEnvironment,
    sandbox: &str,
    project: &ProjectId,
) -> Result<()> {
    let url = git::https_remote_url(project.owner(), project.repository());
    let outcome = sandbox::exec(host, sandbox, &["git", "ls-remote", &url, "HEAD"])?;
    if outcome.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&outcome.stderr);
    if !REJECTED.iter().any(|phrase| stderr.contains(phrase)) {
        return outcome.require_success().map(|_| ());
    }
    Err(Error::single(
        Diagnostic::new(
            ErrorId::GithubCredentialRejected,
            msg!(
                "error-github-credential-rejected",
                sandbox = sandbox,
                repository = format!("{}/{}", project.owner(), project.repository())
            ),
        )
        .fact(Fact::sandbox(sandbox))
        .fact(Fact::reason(msg!(
            "cause-github-credential-rejected",
            service = GITHUB_SERVICE
        )))
        .external(outcome.failure())
        .remediation(
            Remediation::text(msg!("remediation-github-credential-rejected"))
                .try_run(forget_command(sandbox))
                .try_run(register_command(sandbox)),
        ),
    ))
}
