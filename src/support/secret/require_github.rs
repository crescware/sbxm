use crate::boundary::host::HostEnvironment;
use crate::design::{Fact, Remediation};
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;

use super::{GITHUB_HOSTS, GITHUB_TOKEN_ENV, forget_command, register_command, registered_github};

/// `GitHubのcustom` secretが登録済みであることを確認し、そのplaceholderを返す。
///
/// 未登録なら、発行条件と登録commandを示して前提条件不足として停止する。返すのは
/// tokenではなくplaceholderであり、`sbx secret ls`が誰にでも示す公開の値である。
pub fn require_github(host: &dyn HostEnvironment, sandbox: &str) -> Result<String> {
    let registered = registered_github(host, sandbox)?;
    match registered.as_slice() {
        [single] => Ok(single.placeholder.clone()),
        [] => Err(missing(host, sandbox)),
        several => Err(ambiguous(sandbox, several.len())),
    }
}

/// 覆われていないhostだけを示す。github.comだけ登録済みの状態から来た場合に、何が
/// 足りないのかがそのまま読める。どのhostも登録はされていて、1件にまとまっていない
/// だけの場合は、まとめる対象として全hostを示す。
fn missing(host: &dyn HostEnvironment, sandbox: &str) -> Error {
    let customs = super::list_customs(host).unwrap_or_default();
    let uncovered: Vec<&str> = GITHUB_HOSTS
        .iter()
        .filter(|wanted| {
            !customs
                .iter()
                .any(|custom| custom.targets.iter().any(|target| target == *wanted))
        })
        .copied()
        .collect();
    let uncovered = if uncovered.is_empty() {
        GITHUB_HOSTS.to_vec()
    } else {
        uncovered
    };

    // 同じenvのsecretが既にあると、placeholderを指定しない登録は重複として拒否される。
    // 案内どおりに実行しても失敗する状態を作らないため、既存のplaceholderを引き継ぐ形で
    // 示す。同じ値のまま更新されるので、Sandboxを作り直さずに済む。
    let existing = customs
        .iter()
        .find(|custom| custom.env == GITHUB_TOKEN_ENV && custom.scope == sandbox)
        .map(|custom| custom.placeholder.as_str());
    let explanation = if existing.is_some() {
        msg!("remediation-github-secret-incomplete")
    } else {
        msg!("remediation-github-secret-missing")
    };

    Error::single(
        Diagnostic::new(
            ErrorId::GithubSecretMissing,
            msg!(
                "error-github-secret-missing",
                sandbox = sandbox,
                hosts = uncovered.join(", ")
            ),
        )
        .remediation(Remediation::text(explanation).try_run(register_command(sandbox, existing))),
    )
}

/// どのplaceholderを使うべきか決められない。選ばずに止める。
fn ambiguous(sandbox: &str, count: usize) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::GithubSecretMissing,
            msg!(
                "error-github-secret-missing",
                sandbox = sandbox,
                hosts = GITHUB_HOSTS.join(", ")
            ),
        )
        .fact(Fact::reason(msg!(
            "cause-github-secret-ambiguous",
            count = count
        )))
        .remediation(
            Remediation::text(msg!("remediation-github-secret-ambiguous"))
                .try_run(forget_command(sandbox, "<placeholder>")),
        ),
    )
}
