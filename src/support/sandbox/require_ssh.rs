use crate::boundary::host::{CommandSpec, EnvPolicy, HostEnvironment, TimeoutClass};
use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;

use super::ssh_host;

/// `sandbox`へsshでつながる設定があることを、接続せずに確かめる。
///
/// sbxが用意するProxyCommandが、`<sandbox>.sbx`をSandboxへつなぐ。`ssh -G`は接続せず、
/// その宛先に対する実効設定だけを表示する。判定は`status --global`のRemote SSHと同じで
/// ある。hostにあるrepositoryは、このsshでSandboxのoriginを書き込むため、Sandboxを作る
/// 前に確かめる。作ってから届かないと分かるのでは遅い。
pub fn require_ssh(host: &dyn HostEnvironment, sandbox: &str) -> Result<()> {
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
    let configured = outcome
        .stdout_text()
        .lines()
        .any(|line| line.trim().to_ascii_lowercase().starts_with("proxycommand"));
    if configured {
        return Ok(());
    }
    Err(Error::single(
        Diagnostic::new(
            ErrorId::RemoteSshUnconfigured,
            msg!("error-remote-ssh-unconfigured", host = destination),
        )
        .remediation(msg!("remediation-remote-ssh-unconfigured")),
    ))
}
