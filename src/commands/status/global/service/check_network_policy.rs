use crate::boundary::host::HostEnvironment;
use crate::boundary::host::protocol::{NetworkPolicyState, parse_network_policy};
use crate::design::Fact;
use crate::diagnostics::{Diagnostic, ErrorId};
use crate::msg;

use crate::support::StatusValue;

use crate::commands::status::global::external::{describe, external_of, read_stdout};
use crate::commands::status::global::{GlobalStatus, push};

/// policy一覧とgovernanceの可用性を確認する。preset名は一覧から推測しない。
pub fn check_network_policy(host: &dyn HostEnvironment, status: &mut GlobalStatus) {
    match read_stdout(host, "sbx", &["policy", "ls", "--json"]) {
        Ok(output) => match parse_network_policy(&output) {
            Ok(NetworkPolicyState::Available) => {
                push(status, "status-item-network-policy", StatusValue::Ready);
            }
            Ok(NetworkPolicyState::Blocked) => {
                push(status, "status-item-network-policy", StatusValue::Error);
                status.diagnostics.push(
                    Diagnostic::new(
                        ErrorId::NetworkPolicyBlocked,
                        msg!("error-network-policy-blocked"),
                    )
                    .remediation(msg!("remediation-network-policy")),
                );
            }
            Err(error) => {
                push(status, "status-item-network-policy", StatusValue::Error);
                status.diagnostics.push(
                    Diagnostic::new(
                        ErrorId::NetworkPolicyUnobservable,
                        msg!("error-network-policy-unobservable"),
                    )
                    .fact(Fact::cause(&describe(&error))),
                );
            }
        },
        Err(error) => {
            push(status, "status-item-network-policy", StatusValue::Error);
            let mut diagnostic = Diagnostic::new(
                ErrorId::NetworkPolicyUnobservable,
                msg!("error-network-policy-unobservable"),
            )
            .fact(Fact::cause(&describe(&error)));
            if let Some(external) = external_of(&error) {
                diagnostic = diagnostic.external(external);
            }
            status.diagnostics.push(diagnostic);
        }
    }
}
