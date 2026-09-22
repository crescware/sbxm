use crate::boundary::host::HostEnvironment;
use crate::design::{Fact, Remediation};
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::git;
use crate::msg;
use crate::project::ProjectId;

use crate::support::sandbox;

use super::{GithubRegistration, register_command};

/// GitHubが認証を拒んだときに、gitがstderrへ書く文。
///
/// tokenが無効でも、proxyが差し替えずplaceholderがそのまま届いても、GitHubは同じ文で
/// 拒む。到達できない、repositoryが無いなど、認証以外の失敗はこの文を含まない。
const REJECTED: [&str; 3] = [
    "Authentication failed",
    "Invalid username or token",
    "could not read Username",
];

/// 端末を持たない実行でgitが入力を待たないようにしてから、認証だけを1回試す。
const PROBE: &str = r#"GIT_TERMINAL_PROMPT=0 exec git ls-remote "$1" HEAD"#;

/// Sandboxの中のgitが、このrepositoryへ実際に認証できることを確かめる。
///
/// 登録されていることと、gitが提示した値をproxyが差し替えることは別である。tokenの
/// 期限切れ、対象repositoryへの権限不足、proxyが覆っていないhostは、どれも登録の
/// 観測では現れない。数分かかるfetchへ進む前に、実物と同じ経路で1回だけ問い合わせ、
/// 拒まれたときは何を登録し直せばよいかを示す。
/// `registration`はcredential helperへ設定した登録のscopeとplaceholderを受け取り、更新
/// commandでも両方を維持する。一覧を読み直さず、実際に認証を試した登録を更新対象にする。
pub fn require_github_accepts(
    host: &dyn HostEnvironment,
    sandbox: &str,
    project: &ProjectId,
    registration: &GithubRegistration,
) -> Result<()> {
    let url = git::https_remote_url(project.owner(), project.repository());
    let outcome = sandbox::exec(host, sandbox, &["sh", "-c", PROBE, "sh", &url])?;
    if outcome.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&outcome.stderr);
    if !REJECTED.iter().any(|phrase| stderr.contains(phrase)) {
        // 認証以外の失敗は、tokenを登録し直しても直らない。原文のまま報告する。
        return outcome.require_success().map(|_| ());
    }
    Err(Error::single(
        Diagnostic::new(
            ErrorId::GithubCredentialRejected,
            msg!("error-github-credential-rejected", sandbox = sandbox),
        )
        .fact(Fact::sandbox(sandbox))
        .external(outcome.failure())
        .remediation(
            Remediation::text(msg!("remediation-github-credential-rejected")).try_run(
                register_command(registration.scope(), Some(registration.placeholder())),
            ),
        ),
    ))
}
