use crate::boundary::host::{CommandSpec, EnvPolicy, HostEnvironment, TimeoutClass};
use crate::design::Remediation;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;

use super::{require_ssh, ssh_host};

/// 設定とSandboxの存在・起動を確認済みの宛先へ、端末を渡さず実際にSSHで届くことを確かめる。
///
/// 存在しない名前へは呼ばない。接続によるSandboxの暗黙の作成・起動を避け、既存の
/// multiplex接続を再利用せずendpointまで確認する。Gitやworktreeは読まない。
pub fn probe_ssh(host: &dyn HostEnvironment, sandbox: &str) -> Result<()> {
    let destination = ssh_host(sandbox);
    let spec = CommandSpec::probe(
        "ssh",
        &[
            "-n",
            "-T",
            "-o",
            "BatchMode=yes",
            "-o",
            "ForwardAgent=no",
            "-o",
            "ClearAllForwardings=yes",
            "-o",
            "ControlPath=none",
            "-o",
            "ControlMaster=no",
            "-o",
            "StrictHostKeyChecking=yes",
            "-o",
            "UpdateHostKeys=no",
            &destination,
            "true",
        ],
    )
    .env(EnvPolicy::InheritWithoutSshAgent)
    .timeout(TimeoutClass::Probe);
    let outcome = host.run(&spec).map_err(|error| {
        let Error::Diagnostics(mut diagnostics) = error else {
            return error;
        };
        for diagnostic in &mut diagnostics {
            let previous = diagnostic.description.clone();
            diagnostic
                .facts
                .push(crate::design::Fact::cause(diagnostic.id.as_str()));
            diagnostic.id = ErrorId::RemoteSshConnectionFailed;
            diagnostic.description = msg!("error-remote-ssh-connection-failed");
            diagnostic.remediation = Some(
                Remediation::text(msg!("remediation-remote-ssh-connection"))
                    .explain(previous)
                    .try_run(format!("ssh {destination} true")),
            );
        }
        Error::Diagnostics(diagnostics)
    })?;
    if outcome.success() {
        return Ok(());
    }
    // 準備から変わった設定を、失敗時だけ読み直して原因を区別する。
    require_ssh(host, sandbox)?;
    let unavailable = String::from_utf8_lossy(&outcome.stderr)
        .contains("sandboxd SSH endpoint is disabled or unavailable");
    let (id, description) = if unavailable {
        (
            ErrorId::RemoteSshUnavailable,
            msg!("error-remote-ssh-unavailable"),
        )
    } else {
        (
            ErrorId::RemoteSshConnectionFailed,
            msg!("error-remote-ssh-connection-failed"),
        )
    };
    let mut remediation = Remediation::text(msg!("remediation-remote-ssh-connection"));
    if unavailable {
        remediation = remediation
            .explain(msg!("remediation-remote-ssh-restart"))
            .try_run("sbx daemon restart");
    }
    remediation = remediation.try_run(format!("ssh {destination} true"));
    Err(Error::single(
        Diagnostic::new(id, description)
            .remediation(remediation)
            .external(outcome.failure()),
    ))
}
