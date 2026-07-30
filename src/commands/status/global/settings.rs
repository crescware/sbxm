//! global設定とbase pathの診断。

use std::path::Path;

use crate::config::{self, ConfigLocation, ConfigState};
use crate::error::{Diagnostic, ErrorId};
use crate::msg;
use crate::paths;

use crate::support::StatusValue;

use super::{GlobalStatus, push};
use crate::ui::Remediation;

pub(super) fn check_config(
    location: &ConfigLocation,
    status: &mut GlobalStatus,
) -> Option<Box<config::GlobalConfig>> {
    match config::load(location) {
        Ok(ConfigState::Valid { config, warnings }) => {
            push(status, "status-item-config", StatusValue::Ready);
            status.warnings.extend(warnings);
            Some(config)
        }
        Ok(ConfigState::Missing) => {
            push(status, "status-item-config", StatusValue::Missing);
            status.diagnostics.push(
                Diagnostic::new(
                    ErrorId::ConfigMissing,
                    msg!(
                        "error-config-missing",
                        path = paths::display(&location.config_file())
                    ),
                )
                .remediation(Remediation::text(msg!("remediation-run-init")).try_run("sbxm init")),
            );
            None
        }
        Err(error) => {
            push(status, "status-item-config", StatusValue::Error);
            status
                .diagnostics
                .extend(error.diagnostics().iter().cloned());
            None
        }
    }
}

pub(super) fn check_base_path(config: Option<&config::GlobalConfig>, status: &mut GlobalStatus) {
    let Some(config) = config else {
        // configを読めない場合、base pathは宣言自体が存在しない。
        push(status, "status-item-base-path", StatusValue::Missing);
        return;
    };

    let path = config.base_path.as_path();
    match std::fs::metadata(path) {
        Ok(metadata) if metadata.is_dir() => {
            if is_writable_dir(path) {
                push(status, "status-item-base-path", StatusValue::Ready);
            } else {
                push(status, "status-item-base-path", StatusValue::Error);
                status.diagnostics.push(Diagnostic::new(
                    ErrorId::BasePathNotWritable,
                    msg!("error-base-path-not-writable", path = paths::display(path)),
                ));
            }
        }
        Ok(_) => {
            push(status, "status-item-base-path", StatusValue::Error);
            status.diagnostics.push(Diagnostic::new(
                ErrorId::BasePathNotDirectory,
                msg!("error-base-path-not-directory", path = paths::display(path)),
            ));
        }
        Err(_) => {
            // `add`が作成するため、未作成であること自体はerrorではない。
            push(status, "status-item-base-path", StatusValue::Missing);
        }
    }
}

pub(super) fn is_writable_dir(path: &Path) -> bool {
    let probe = path.join(".sbxm-write-probe");
    match std::fs::create_dir(&probe) {
        Ok(()) => {
            let _ = std::fs::remove_dir(&probe);
            true
        }
        Err(error) => error.kind() == std::io::ErrorKind::AlreadyExists,
    }
}
