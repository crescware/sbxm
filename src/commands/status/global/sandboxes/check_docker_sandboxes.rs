use crate::boundary::host::HostEnvironment;
use crate::boundary::host::protocol::{CliVersion, require_minimum_version};
use crate::design::ProgressSink;
use crate::diagnostics::{Diagnostic, ErrorId};
use crate::msg;

use crate::support::StatusValue;

use crate::commands::status::global::external::{describe, external_of, read_stdout};
use crate::commands::status::global::service::{check_daemon, check_login, check_network_policy};
use crate::commands::status::global::{GlobalStatus, push};

use super::check_remote_ssh;

/// Docker Sandboxes CLIのversion、network policy、daemonの状態。
pub fn check_docker_sandboxes(
    host: &dyn HostEnvironment,
    sbx_present: bool,
    status: &mut GlobalStatus,
    progress: &mut dyn ProgressSink,
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
                msg!("error-host-command-missing", program = "sbx"),
            )
            .remediation(msg!("remediation-install-command", program = "sbx")),
        );
        for item in dependent_items {
            push(status, item, StatusValue::Error);
        }
        return;
    }

    progress.step(msg!("progress-inspect-sbx-version"));
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
    progress.step(msg!("progress-inspect-network"));
    check_network_policy(host, status);
    progress.step(msg!("progress-inspect-daemon"));
    check_daemon(host, status);
    check_login(host, status, progress);
    progress.step(msg!("progress-inspect-remote-ssh"));
    check_remote_ssh(host, status);
}
