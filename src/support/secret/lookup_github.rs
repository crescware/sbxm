use crate::boundary::host::HostEnvironment;
use crate::design::{Fact, Remediation};
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;

use super::{
    GITHUB_HOSTS, GITHUB_TOKEN_ENV, GithubLookup, GithubRegistration, forget_command,
    register_command, registered_github,
};

/// このSandboxが使える`GitHub`の`custom secret`登録を探す。
///
/// 1件も無い場合と、1件に決まらない場合は、それぞれ発行条件と登録command、または
/// 余分な登録を消すcommandを示す診断を持たせて返す。返すscopeとplaceholderは
/// `sbx secret ls`が示す公開の値であり、tokenそのものは持たない。`Err`は一覧そのものを
/// 読めなかった場合だけである。
pub fn lookup_github(host: &dyn HostEnvironment, sandbox: &str) -> Result<GithubLookup> {
    let registered = registered_github(host, sandbox)?;
    Ok(match registered.as_slice() {
        [single] => {
            GithubLookup::Registered(GithubRegistration::new(&single.scope, &single.placeholder))
        }
        [] => GithubLookup::Unregistered(missing(host, sandbox)),
        several => GithubLookup::Ambiguous(ambiguous(sandbox, several.len())),
    })
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
