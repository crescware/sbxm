use crate::boundary::host::HostEnvironment;
use crate::boundary::host::protocol::parse_login_status;
use crate::diagnostics::{Diagnostic, ErrorId};
use crate::msg;

use crate::support::StatusValue;

use crate::commands::status::global::external::{describe, external_of, read_stdout};
use crate::commands::status::global::{GlobalStatus, push};
use crate::design::{Fact, Remediation};

/// Docker Sandboxesへのlogin状態。
///
/// loginを前提とするのはTemplateとSandboxを扱う工程であり、observeできない場合に
/// login済みと推測しない。
pub fn check_login(host: &dyn HostEnvironment, status: &mut GlobalStatus) {
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
                msg!("error-sbx-login-unobservable"),
            )
            .fact(Fact::cause(&describe(&error)));
            if let Some(external) = external_of(&error) {
                diagnostic = diagnostic.external(external);
            }
            status.diagnostics.push(diagnostic);
        }
    }
}
