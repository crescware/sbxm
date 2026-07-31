use crate::command::HostEnvironment;
use crate::diagnostics::{Diagnostic, ErrorId};
use crate::msg;

use crate::support::StatusValue;

use crate::commands::status::global::external::{describe, external_of, read_stdout};
use crate::commands::status::global::{GlobalStatus, push};

/// Remote `SSHでSandboxへ接続できる設定になっているか`。
///
/// `open`は`<sandbox-name>.sbx`へsshするため、その名前をsshが解決できることを
/// read-onlyで確かめる。設定方法は対象versionごとに異なるため、実機で確認していない
/// setup commandは案内しない。
pub fn check_remote_ssh(host: &dyn HostEnvironment, status: &mut GlobalStatus) {
    // `ssh -G`は接続せず、その宛先に対する実効設定だけを表示する。
    match read_stdout(host, "ssh", &["-G", "sbxm-probe.sbx"]) {
        Ok(output) => {
            let configured = output
                .lines()
                .any(|line| line.trim().to_ascii_lowercase().starts_with("proxycommand"));
            if configured {
                push(status, "status-item-remote-ssh", StatusValue::Ready);
            } else {
                push(status, "status-item-remote-ssh", StatusValue::Missing);
                status.diagnostics.push(
                    Diagnostic::new(
                        ErrorId::RemoteSshUnconfigured,
                        msg!("error-remote-ssh-unconfigured", host = "*.sbx"),
                    )
                    .remediation(msg!("remediation-remote-ssh-unconfigured")),
                );
            }
        }
        Err(error) => {
            push(status, "status-item-remote-ssh", StatusValue::Error);
            let mut diagnostic = Diagnostic::new(
                ErrorId::RemoteSshUnobservable,
                msg!("error-remote-ssh-unobservable", detail = describe(&error)),
            );
            if let Some(external) = external_of(&error) {
                diagnostic = diagnostic.external(external);
            }
            status.diagnostics.push(diagnostic);
        }
    }
}
