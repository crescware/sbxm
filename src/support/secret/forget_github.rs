use crate::boundary::host::protocol::{CustomSecret, SecretListing};
use crate::boundary::host::{CommandSpec, EnvPolicy, HostEnvironment, TimeoutClass};
use crate::design::{Fact, Remediation};
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;

use super::{
    GITHUB_SERVICE, GITHUB_TOKEN_ENV, forget_command, forget_custom_command, list_secrets,
};

/// このscopeへ結び付いた、sbxmが案内したtokenの登録。
///
/// scopeが一致するものだけを選ぶ。global scopeのsecretはほかのSandboxも使うため、
/// 1案件の後片付けで消してよい対象ではない。以前の版が案内したcustom secretも、
/// このscopeへ`GH_TOKEN`を運ぶものは同じく片付ける。利用者が同じscopeへ別のsecretを
/// 登録していることがあるため、名前とenvを見る。
struct Registered {
    service: bool,
    customs: Vec<CustomSecret>,
}

impl Registered {
    fn in_scope(listing: SecretListing, sandbox: &str) -> Registered {
        Registered {
            service: listing
                .services
                .iter()
                .any(|service| service.scope == sandbox && service.name == GITHUB_SERVICE),
            customs: listing
                .customs
                .into_iter()
                .filter(|custom| custom.scope == sandbox && custom.env == GITHUB_TOKEN_ENV)
                .collect(),
        }
    }

    fn is_empty(&self) -> bool {
        !self.service && self.customs.is_empty()
    }

    /// 案内する文字列と実行する引数がずれると、対処方法どおりに実行しても結果が変わる。
    /// 実行も案内も、この1つの一覧から作る。
    fn commands(&self, sandbox: &str) -> Vec<String> {
        let mut commands = Vec::new();
        if self.service {
            commands.push(forget_command(sandbox));
        }
        for custom in &self.customs {
            commands.push(forget_custom_command(sandbox, &custom.placeholder));
        }
        commands
    }
}

/// Sandboxを消したあと、そのscopeに残るtokenの登録も解く。
///
/// Docker Sandboxes v0.42.0からは`sbx rm`がSandbox限定のsecretも消すが、それに頼らず
/// 一覧を読み直して確かめる。消したSandbox宛のtokenを預けたままにしないためである。
/// commandの戻り値だけを不在の根拠にしない。
pub fn forget_github(host: &dyn HostEnvironment, sandbox: &str) -> Result<()> {
    let registered = Registered::in_scope(list_secrets(host)?, sandbox);
    if registered.is_empty() {
        return Ok(());
    }
    for command in registered.commands(sandbox) {
        let args: Vec<&str> = command.split(' ').skip(1).collect();
        let spec = CommandSpec::capture("sbx", &args)
            .env(EnvPolicy::InheritWithoutSshAgent)
            .timeout(TimeoutClass::SandboxLifecycle);
        host.run(&spec)?.require_success()?;
    }
    let left = Registered::in_scope(list_secrets(host)?, sandbox);
    if left.is_empty() {
        return Ok(());
    }
    let mut remediation = Remediation::text(msg!("remediation-secret-still-registered"));
    for command in left.commands(sandbox) {
        remediation = remediation.try_run(command);
    }
    Err(Error::single(
        Diagnostic::new(
            ErrorId::SecretStillRegistered,
            msg!("error-secret-still-registered", sandbox = sandbox),
        )
        .fact(Fact::sandbox(sandbox))
        .remediation(remediation),
    ))
}
