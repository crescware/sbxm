//! global stateの診断。
//!
//! configとregistryはread-onlyで観測する。不在は正常であり、自動作成も自動修復も
//! 行わない。登録案件を巡回しないため、登録数によって実行時間と出力行数が増えない。

use std::path::Path;

use crate::config::{self, ConfigLocation, ConfigState, GlobalConfig};
use crate::error::{Diagnostic, ErrorId};
use crate::msg;
use crate::paths;
use crate::registry;

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
///
/// 読めた設定は、後続の診断が同じfileをもう一度読まずに済むよう返す。読めなければ
/// `None`とし、その事実はここで一度だけ報告する。
pub(super) fn check_config(
    location: &ConfigLocation,
    status: &mut GlobalStatus,
) -> Option<GlobalConfig> {
    let (value, config) = match config::load(location) {
        Ok(ConfigState::Valid { config, warnings }) => {
            status.warnings.extend(warnings);
            (StatusValue::Ready, Some(*config))
        }
        Ok(ConfigState::Missing) => (StatusValue::Defaults, Some(GlobalConfig::default())),
        Err(error) => {
            status
                .diagnostics
                .extend(error.diagnostics().iter().cloned());
            (StatusValue::Error, None)
        }
    };
    push(status, "status-item-config", value);
    config
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

/// 新規登録の既定として保存されているGit identity。
///
/// hostの設定は見ない。既定を選ぶのは利用者であり、未保存であることは、対話的な
/// `add`がまだ一度も訊いていないことを意味する。errorではないため案内も出さない。
///
/// configそのものが読めない場合は`check_config`が既に報告しているため、ここでは
/// 同じ事実を重ねて報告しない。
pub(super) fn check_git_identity(config: Option<&GlobalConfig>, status: &mut GlobalStatus) {
    let value = match config {
        Some(config) if config.git_identity.is_some() => StatusValue::Ready,
        Some(_) => StatusValue::Missing,
        None => StatusValue::Error,
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
