use crate::boundary::host::HostEnvironment;
use crate::boundary::host::protocol::CustomSecret;
use crate::design::Remediation;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;

use super::{GITHUB_HOSTS, GITHUB_TOKEN_ENV, list_customs, register_command};

/// `GitHubのcustom` secretが登録済みであることを確認する。
///
/// 未登録なら、発行条件と登録commandを示して前提条件不足として停止する。custom secretは
/// Sandboxの作成時に結び付くため、この確認はSandboxを作る前に行う。
pub fn require_github(host: &dyn HostEnvironment, sandbox: &str) -> Result<()> {
    let customs = list_customs(host, sandbox)?;

    // 1件のsecretが全hostを覆っていることを求める。複数のsecretへ分けて登録すると
    // placeholderが分かれ、Sandboxはそのうち1つしか受け取れない。
    //
    // scopeは絞らない。global scopeの登録でもSandboxはplaceholderを受け取れる。
    let covered = |custom: &CustomSecret| {
        custom.env == GITHUB_TOKEN_ENV
            && GITHUB_HOSTS
                .iter()
                .all(|host| custom.targets.iter().any(|target| target == host))
    };
    if customs.iter().any(covered) {
        return Ok(());
    }

    // 覆われていないhostだけを示す。github.comだけ登録済みの状態から来た場合に、
    // 何が足りないのかがそのまま読める。どのhostも登録はされていて、1件にまとまって
    // いないだけの場合は、まとめる対象として全hostを示す。
    let missing: Vec<&str> = GITHUB_HOSTS
        .iter()
        .filter(|host| {
            !customs.iter().any(|custom| {
                custom.env == GITHUB_TOKEN_ENV
                    && custom.targets.iter().any(|target| target == *host)
            })
        })
        .copied()
        .collect();
    let missing = if missing.is_empty() {
        GITHUB_HOSTS.to_vec()
    } else {
        missing
    };

    // 同じenvのsecretが既にあると、placeholderを指定しない登録は重複として拒否される。
    // 案内どおりに実行しても失敗する状態を作らないため、既存のplaceholderを引き継ぐ形で
    // 示す。同じ値のまま更新されるので、既存Sandboxを作り直さずに済む。
    let existing = customs
        .iter()
        .find(|custom| custom.env == GITHUB_TOKEN_ENV)
        .map(|custom| custom.placeholder.as_str());

    Err(Error::single(
        Diagnostic::new(
            ErrorId::GithubSecretMissing,
            msg!(
                "error-github-secret-missing",
                sandbox = sandbox,
                hosts = missing.join(", ")
            ),
        )
        .remediation(match existing {
            Some(placeholder) => Remediation::text(msg!("remediation-github-secret-incomplete"))
                .try_run(register_command(sandbox, Some(placeholder))),
            None => Remediation::text(msg!("remediation-github-secret-missing"))
                .try_run(register_command(sandbox, None)),
        }),
    ))
}
