//! Docker Sandboxes CLIと、その周辺の診断。

use crate::command::HostEnvironment;
use crate::compatibility::{CliVersion, require_minimum_version};
use crate::error::{Diagnostic, ErrorId};
use crate::msg;

use crate::support::StatusValue;

use super::external::{describe, external_of, read_stdout};
use super::service::{check_daemon, check_login, check_network_policy};
use super::{GlobalStatus, push};

/// Docker Sandboxes CLIのversion、network policy、daemonの状態。
pub(super) fn check_docker_sandboxes(
    host: &dyn HostEnvironment,
    sbx_present: bool,
    status: &mut GlobalStatus,
) {
    let dependent_items = [
        "status-item-network-policy",
        "status-item-daemon",
        "status-item-login",
        "status-item-remote-ssh",
    ];

    if !sbx_present {
        push(status, "status-item-docker-sandboxes", StatusValue::Missing);
        status.diagnostics.push(
            Diagnostic::new(
                ErrorId::HostCommandMissing,
                msg!("error-host-command-missing", command = "sbx"),
            )
            .remediation(msg!("remediation-install-command", command = "sbx")),
        );
        for item in dependent_items {
            push(status, item, StatusValue::Error);
        }
        return;
    }

    let output = match read_stdout(host, "sbx", &["version"]) {
        Ok(output) => output,
        Err(error) => {
            push(status, "status-item-docker-sandboxes", StatusValue::Error);
            let mut diagnostic = Diagnostic::new(
                ErrorId::SbxVersionUnparseable,
                msg!("error-sbx-version-unparseable", observed = describe(&error)),
            );
            if let Some(external) = external_of(&error) {
                diagnostic = diagnostic.external(external);
            }
            status.diagnostics.push(diagnostic);
            for item in dependent_items {
                push(status, item, StatusValue::Error);
            }
            return;
        }
    };

    let Some(observed) = CliVersion::extract_from_output(&output) else {
        push(status, "status-item-docker-sandboxes", StatusValue::Error);
        status.diagnostics.push(Diagnostic::new(
            ErrorId::SbxVersionUnparseable,
            msg!("error-sbx-version-unparseable", observed = output.trim()),
        ));
        for item in dependent_items {
            push(status, item, StatusValue::Error);
        }
        return;
    };

    if let Err(error) = require_minimum_version(observed) {
        push(status, "status-item-docker-sandboxes", StatusValue::Error);
        status
            .diagnostics
            .extend(error.diagnostics().iter().cloned());
        for item in dependent_items {
            push(status, item, StatusValue::Error);
        }
        return;
    }

    push(status, "status-item-docker-sandboxes", StatusValue::Ready);
    check_network_policy(host, status);
    check_daemon(host, status);
    check_login(host, status);
    check_remote_ssh(host, status);
}

/// Remote SSHでSandboxへ接続できる設定になっているか。
///
/// `open`は`<sandbox-name>.sbx`へsshするため、その名前をsshが解決できることを
/// read-onlyで確かめる。設定方法は対象versionごとに異なるため、実機で確認していない
/// setup commandは案内しない。
pub(super) fn check_remote_ssh(host: &dyn HostEnvironment, status: &mut GlobalStatus) {
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
