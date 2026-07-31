//! global stateの診断。
//!
//! configとregistryはread-onlyで観測する。不在は正常であり、自動作成も自動修復も
//! 行わない。登録案件を巡回しないため、登録数によって実行時間と出力行数が増えない。

use std::path::Path;

use crate::command::HostEnvironment;
use crate::config::{self, ConfigLocation, ConfigState};
use crate::error::{Diagnostic, ErrorId};
use crate::msg;
use crate::paths;
use crate::registry;
use crate::support::identity;

use crate::support::StatusValue;

use super::{GlobalStatus, push};

/// `~/.sbxm`そのものの読み書き可否。
pub(super) fn check_state_directory(location: &ConfigLocation, status: &mut GlobalStatus) {
    let dir = location.dir();
    let value = match std::fs::symlink_metadata(&dir) {
        // 未作成であること自体はerrorではない。最初の登録が作る。
        Err(_) => StatusValue::Missing,
        Ok(metadata) if !metadata.is_dir() => {
            status.diagnostics.push(Diagnostic::new(
                ErrorId::GlobalStateUnusable,
                msg!(
                    "error-global-state-unusable",
                    path = paths::display(&dir),
                    detail = "the path is not a directory"
                ),
            ));
            StatusValue::Error
        }
        Ok(_) if !is_writable_dir(&dir) => {
            status.diagnostics.push(Diagnostic::new(
                ErrorId::GlobalStateUnusable,
                msg!(
                    "error-global-state-unusable",
                    path = paths::display(&dir),
                    detail = "the directory is not writable by the current user"
                ),
            ));
            StatusValue::Error
        }
        Ok(_) => StatusValue::Ready,
    };
    push(status, "status-item-state-directory", value);
}

/// 任意のglobal config。不在は`defaults`として正常に扱う。
pub(super) fn check_config(location: &ConfigLocation, status: &mut GlobalStatus) {
    let value = match config::load(location) {
        Ok(ConfigState::Valid { warnings, .. }) => {
            status.warnings.extend(warnings);
            StatusValue::Ready
        }
        Ok(ConfigState::Missing) => StatusValue::Defaults,
        Err(error) => {
            status
                .diagnostics
                .extend(error.diagnostics().iter().cloned());
            StatusValue::Error
        }
    };
    push(status, "status-item-config", value);
}

/// registry documentのversion、構文、permission、不変条件。
///
/// 不在は登録案件0件として正常に扱う。個々の案件へは触れない。
pub(super) fn check_registry(location: &ConfigLocation, status: &mut GlobalStatus) {
    let value = match registry::load(location) {
        Ok(registry) if registry.entries().is_empty() => StatusValue::Missing,
        Ok(_) => StatusValue::Ready,
        Err(error) => {
            status
                .diagnostics
                .extend(error.diagnostics().iter().cloned());
            StatusValue::Error
        }
    };
    push(status, "status-item-registry", value);
}

/// 新規登録に使えるhostのGit identity。
pub(super) fn check_git_identity(host: &dyn HostEnvironment, status: &mut GlobalStatus) {
    let value = match identity::from_host(host) {
        Ok(_) => StatusValue::Ready,
        Err(error) => {
            status
                .diagnostics
                .extend(error.diagnostics().iter().cloned());
            StatusValue::Missing
        }
    };
    push(status, "status-item-git-identity", value);
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
