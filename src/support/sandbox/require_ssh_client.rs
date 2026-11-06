use super::{proxy_command_configured, ssh_host};
use crate::boundary::host::{CommandSpec, EnvPolicy, HostEnvironment, TimeoutClass};
use crate::design::{Fact, Remediation};
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;

/// clientの実効設定だけを読む。featureの観測を持つ修復でも同じ判定を使う。
pub(crate) fn require_ssh_client(host: &dyn HostEnvironment, sandbox: &str) -> Result<()> {
    let destination = ssh_host(sandbox);
    let spec = CommandSpec::probe("ssh", &["-G", &destination])
        .env(EnvPolicy::InheritWithoutSshAgent)
        .timeout(TimeoutClass::Probe);
    let outcome = host.run(&spec)?;
    if !outcome.success() {
        let stderr = String::from_utf8_lossy(&outcome.stderr);
        return Err(Error::single(
            Diagnostic::new(
                ErrorId::RemoteSshUnobservable,
                msg!("error-remote-ssh-unobservable"),
            )
            .fact(Fact::cause(stderr.trim())),
        ));
    }
    if proxy_command_configured(&outcome.stdout_text()) {
        return Ok(());
    }
    Err(Error::single(
        Diagnostic::new(
            ErrorId::RemoteSshUnconfigured,
            msg!("error-remote-ssh-unconfigured", host = destination),
        )
        .remediation(
            Remediation::text(msg!("remediation-remote-ssh-unconfigured")).try_run("sbx setup ssh"),
        ),
    ))
}
