use crate::command::HostEnvironment;
use crate::compatibility::{EXPECTED_NETWORK_POLICY, parse_network_policy};
use crate::diagnostics::{Diagnostic, ErrorId};
use crate::msg;

use crate::support::StatusValue;

use crate::commands::status::global::external::{describe, external_of, read_stdout};
use crate::commands::status::global::{GlobalStatus, push};

/// active session検査の対応状況。
///
/// daemonを安全に再起動できるかどうかは、この検査ができるかどうかで決まる。
/// Sandboxが1件もない場合は、sessionを持ち得る対象がないため対応済みとして扱う。
pub fn check_network_policy(host: &dyn HostEnvironment, status: &mut GlobalStatus) {
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
