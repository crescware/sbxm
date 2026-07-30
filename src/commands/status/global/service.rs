//! login、network policy、daemonの診断。

use crate::command::HostEnvironment;
use crate::compatibility::{
    EXPECTED_NETWORK_POLICY, parse_daemon_status, parse_login_status, parse_network_policy,
};
use crate::error::{Diagnostic, ErrorId};
use crate::msg;

use crate::support::StatusValue;

use super::external::{describe, external_of, read_stdout};
use super::{GlobalStatus, push};
use crate::ui::Remediation;

/// Docker Sandboxesへのlogin状態。
///
/// loginを前提とするのはTemplateとSandboxを扱う工程であり、observeできない場合に
/// login済みと推測しない。
pub(super) fn check_login(host: &dyn HostEnvironment, status: &mut GlobalStatus) {
    match read_stdout(host, "sbx", &["login", "status", "--json"]) {
        Ok(output) => match parse_login_status(&output) {
            Ok(true) => push(status, "status-item-login", StatusValue::Ready),
            Ok(false) => {
                push(status, "status-item-login", StatusValue::Missing);
                status.diagnostics.push(
                    Diagnostic::new(ErrorId::SbxLoginMissing, msg!("error-sbx-login-missing"))
                        .remediation(
                            Remediation::text(msg!("remediation-sbx-login")).try_run("sbx login"),
                        ),
                );
            }
            Err(error) => {
                push(status, "status-item-login", StatusValue::Error);
                status
                    .diagnostics
                    .extend(error.diagnostics().iter().cloned());
            }
        },
        Err(error) => {
            push(status, "status-item-login", StatusValue::Error);
            let mut diagnostic = Diagnostic::new(
                ErrorId::SbxLoginUnobservable,
                msg!("error-sbx-login-unobservable", detail = describe(&error)),
            );
            if let Some(external) = external_of(&error) {
                diagnostic = diagnostic.external(external);
            }
            status.diagnostics.push(diagnostic);
        }
    }
}

/// active session検査の対応状況。
///
/// daemonを安全に再起動できるかどうかは、この検査ができるかどうかで決まる。
/// Sandboxが1件もない場合は、sessionを持ち得る対象がないため対応済みとして扱う。
pub(super) fn check_network_policy(host: &dyn HostEnvironment, status: &mut GlobalStatus) {
    match read_stdout(host, "sbx", &["policy", "ls"]) {
        Ok(output) => match parse_network_policy(&output) {
            Ok(observed) if observed == EXPECTED_NETWORK_POLICY => {
                push(status, "status-item-network-policy", StatusValue::Ready);
            }
            Ok(observed) => {
                push(status, "status-item-network-policy", StatusValue::Error);
                status.diagnostics.push(
                    Diagnostic::new(
                        ErrorId::NetworkPolicyMismatch,
                        msg!(
                            "error-network-policy-mismatch",
                            observed = observed,
                            expected = EXPECTED_NETWORK_POLICY
                        ),
                    )
                    .remediation(msg!(
                        "remediation-network-policy",
                        expected = EXPECTED_NETWORK_POLICY
                    )),
                );
            }
            Err(error) => {
                push(status, "status-item-network-policy", StatusValue::Error);
                status.diagnostics.push(Diagnostic::new(
                    ErrorId::NetworkPolicyUnobservable,
                    msg!(
                        "error-network-policy-unobservable",
                        detail = describe(&error)
                    ),
                ));
            }
        },
        Err(error) => {
            push(status, "status-item-network-policy", StatusValue::Error);
            let mut diagnostic = Diagnostic::new(
                ErrorId::NetworkPolicyUnobservable,
                msg!(
                    "error-network-policy-unobservable",
                    detail = describe(&error)
                ),
            );
            if let Some(external) = external_of(&error) {
                diagnostic = diagnostic.external(external);
            }
            status.diagnostics.push(diagnostic);
        }
    }
}

pub(super) fn check_daemon(host: &dyn HostEnvironment, status: &mut GlobalStatus) {
    match read_stdout(host, "sbx", &["daemon", "status"]) {
        Ok(output) => match parse_daemon_status(&output) {
            Ok(state) => {
                let value = match state {
                    crate::compatibility::DaemonState::Running => StatusValue::Running,
                    crate::compatibility::DaemonState::Stopped => StatusValue::Stopped,
                };
                push(status, "status-item-daemon", value);
            }
            Err(error) => {
                push(status, "status-item-daemon", StatusValue::Error);
                status.diagnostics.push(Diagnostic::new(
                    ErrorId::DaemonUnobservable,
                    msg!("error-daemon-unobservable", detail = describe(&error)),
                ));
            }
        },
        Err(error) => {
            push(status, "status-item-daemon", StatusValue::Error);
            let mut diagnostic = Diagnostic::new(
                ErrorId::DaemonUnobservable,
                msg!("error-daemon-unobservable", detail = describe(&error)),
            );
            if let Some(external) = external_of(&error) {
                diagnostic = diagnostic.external(external);
            }
            status.diagnostics.push(diagnostic);
        }
    }
}
