use crate::command::HostEnvironment;
use crate::compatibility::parse_daemon_status;
use crate::diagnostics::{Diagnostic, ErrorId};
use crate::msg;

use crate::support::StatusValue;

use crate::commands::status::global::external::{describe, external_of, read_stdout};
use crate::commands::status::global::{GlobalStatus, push};

pub fn check_daemon(host: &dyn HostEnvironment, status: &mut GlobalStatus) {
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
