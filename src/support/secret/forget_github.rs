use crate::boundary::host::protocol::CustomSecret;
use crate::boundary::host::{CommandSpec, EnvPolicy, HostEnvironment, TimeoutClass};
use crate::design::Remediation;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;

use super::{GITHUB_TOKEN_ENV, covers_github_hosts, forget_command, list_customs};

/// このscopeへ結び付いた、sbxmが案内したtokenの登録。
///
/// scopeが一致するものだけを選ぶ。global scopeのsecretはほかのSandboxも使うため、
/// 1案件の後片付けで消してよい対象ではない。sbxmが案内した形、つまり求めるhostを
/// 1件で覆う登録か、`GH_TOKEN`を運ぶ登録だけを扱う。利用者が同じscopeへ別の用途で
/// 登録したsecretには触れない。
fn scoped_github(customs: Vec<CustomSecret>, sandbox: &str) -> Vec<CustomSecret> {
    customs
        .into_iter()
        .filter(|custom| {
            custom.scope == sandbox
                && (covers_github_hosts(custom) || custom.env == GITHUB_TOKEN_ENV)
        })
        .collect()
}

/// Sandboxを消したあと、そのscopeに残るtokenの登録も解く。
///
/// scopeはSandboxの有無と無関係に残る。Sandboxだけを消すと、次に同じ案件を登録して
/// 案内どおりに`set-custom`を実行しても、同じenvの登録が既にあるとして拒否される。
/// 消したSandbox宛のtokenを預けたままにしないためでもある。
///
/// 消えたことは一覧を読み直して確かめる。commandの戻り値だけを不在の根拠にしない。
/// 返す値は解いた登録のplaceholderであり、tokenそのものは読まない。
pub fn forget_github(host: &dyn HostEnvironment, sandbox: &str) -> Result<Vec<String>> {
    let registered = scoped_github(list_customs(host)?, sandbox);
    if registered.is_empty() {
        return Ok(Vec::new());
    }
    // 同じenvへhostを分けて登録した状態から来ることがある。1件だけ消して残さない。
    //
    // 指定は`forget_command`が示すものと同じにする。案内する文字列と実行する引数が
    // ずれると、対処方法どおりに実行しても結果が変わる。
    for custom in &registered {
        let spec = CommandSpec::capture(
            "sbx",
            &[
                "secret",
                "rm",
                sandbox,
                "--placeholder",
                &custom.placeholder,
                "--force",
            ],
        )
        .env(EnvPolicy::InheritWithoutSshAgent)
        .timeout(TimeoutClass::SandboxLifecycle);
        host.run(&spec)?.require_success()?;
    }
    let left = scoped_github(list_customs(host)?, sandbox);
    if left.is_empty() {
        return Ok(registered
            .into_iter()
            .map(|custom| custom.placeholder)
            .collect());
    }

    let mut remediation = Remediation::text(msg!("remediation-secret-still-registered"));
    for custom in &left {
        remediation = remediation.try_run(forget_command(sandbox, &custom.placeholder));
    }
    Err(Error::single(
        Diagnostic::new(
            ErrorId::SecretStillRegistered,
            msg!(
                "error-secret-still-registered",
                env = GITHUB_TOKEN_ENV,
                sandbox = sandbox
            ),
        )
        .remediation(remediation),
    ))
}
