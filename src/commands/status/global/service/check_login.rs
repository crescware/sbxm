use crate::boundary::host::HostEnvironment;
use crate::diagnostics::ErrorId;

use crate::support::{StatusValue, login};

use crate::commands::status::global::{GlobalStatus, push};

/// Docker Sandboxesへのlogin状態。
///
/// loginを前提とするのはTemplateとSandboxを扱う工程であり、observeできない場合に
/// login済みと推測しない。
pub fn check_login(host: &dyn HostEnvironment, status: &mut GlobalStatus) {
    match login::require_signed_in(host) {
        Ok(()) => push(status, "status-item-login", StatusValue::Ready),
        Err(error) => {
            let value = if error.contains_id(ErrorId::SbxLoginMissing) {
                StatusValue::Missing
            } else {
                StatusValue::Error
            };
            push(status, "status-item-login", value);
            status
                .diagnostics
                .extend(error.diagnostics().iter().cloned());
        }
    }
}
