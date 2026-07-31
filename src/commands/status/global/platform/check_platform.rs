use crate::command::HostEnvironment;
use crate::diagnostics::{Diagnostic, ErrorId};
use crate::msg;

use crate::support::StatusValue;

use crate::commands::status::global::external::{describe, read_stdout};
use crate::commands::status::global::{GlobalStatus, push};

use super::{EXPECTED_ARCHITECTURE, EXPECTED_PLATFORM, MINIMUM_MACOS_MAJOR};

pub fn check_platform(host: &dyn HostEnvironment, status: &mut GlobalStatus) {
    let version = read_stdout(host, "sw_vers", &["-productVersion"]);
    let architecture = read_stdout(host, "uname", &["-m"]);

    match (version, architecture) {
        (Ok(version), Ok(architecture)) => {
            let version = version.trim().to_string();
            let architecture = architecture.trim().to_string();
            let major = version
                .split('.')
                .next()
                .and_then(|value| value.parse::<u32>().ok());
            let observed = format!("macOS {version} on {architecture}");
            match major {
                Some(major)
                    if major >= MINIMUM_MACOS_MAJOR && architecture == EXPECTED_ARCHITECTURE =>
                {
                    push(status, "status-item-platform", StatusValue::Ready);
                }
                Some(_) => {
                    push(status, "status-item-platform", StatusValue::Error);
                    status.diagnostics.push(Diagnostic::new(
                        ErrorId::PlatformUnsupported,
                        msg!(
                            "error-platform-unsupported",
                            expected = EXPECTED_PLATFORM,
                            observed = observed
                        ),
                    ));
                }
                None => {
                    push(status, "status-item-platform", StatusValue::Error);
                    status.diagnostics.push(Diagnostic::new(
                        ErrorId::PlatformUnobservable,
                        msg!(
                            "error-platform-unobservable",
                            detail = format!("the macOS version {version} could not be read")
                        ),
                    ));
                }
            }
        }
        (Err(error), _) | (_, Err(error)) => {
            push(status, "status-item-platform", StatusValue::Error);
            status.diagnostics.push(Diagnostic::new(
                ErrorId::PlatformUnobservable,
                msg!("error-platform-unobservable", detail = describe(&error)),
            ));
        }
    }
}
