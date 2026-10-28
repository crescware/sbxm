use crate::boundary::host::HostEnvironment;
use crate::boundary::host::protocol::{CliVersion, MINIMUM_CLI_VERSION, require_minimum_version};
use crate::config::ConfigLocation;
use crate::design::ProgressSink;
use crate::diagnostics::{Diagnostic, ErrorId};
use crate::msg;

use crate::support::StatusValue;
use crate::support::host_tools;

use crate::commands::status::global::external::{describe, external_of, read_stdout};
use crate::commands::status::global::service::{check_daemon, check_login, check_network_policy};
use crate::commands::status::global::{GlobalStatus, push};

use super::check_remote_ssh;

/// Docker Sandboxes CLIのversion、network policy、daemonの状態。
pub fn check_docker_sandboxes(
    location: &ConfigLocation,
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

    if !cli_version_is_supported(location, host, status, progress) {
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

/// CLIが、このbuildの求める最低version以上か。
///
/// ほかのcommandと同じく、同じ実行ファイルの確認が一度通っていれば記録により読まずに
/// 済ませ、読んで通れば記録する。満たさない場合は項目をErrorにし、診断を積む。
fn cli_version_is_supported(
    location: &ConfigLocation,
    host: &dyn HostEnvironment,
    status: &mut GlobalStatus,
    progress: &mut dyn ProgressSink,
) -> bool {
    let executable = host.inspect_executable("sbx");
    if host_tools::recorded_version(
        location,
        "sbx",
        executable.as_ref(),
        MINIMUM_CLI_VERSION,
        progress,
    )
    .is_some()
    {
        return true;
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
            return false;
        }
    };

    let Some(observed) = CliVersion::extract_from_output(&output) else {
        push(status, "status-item-docker-sandboxes", StatusValue::Error);
        status.diagnostics.push(Diagnostic::new(
            ErrorId::SbxVersionUnparseable,
            msg!("error-sbx-version-unparseable", observed = output.trim()),
        ));
        return false;
    };

    if let Err(error) = require_minimum_version(observed) {
        push(status, "status-item-docker-sandboxes", StatusValue::Error);
        status
            .diagnostics
            .extend(error.diagnostics().iter().cloned());
        return false;
    }

    host_tools::record(location, "sbx", executable.as_ref(), observed, progress);
    true
}
