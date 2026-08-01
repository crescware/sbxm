use crate::config::ConfigLocation;
use crate::design::Fact;
use crate::diagnostics::{Diagnostic, ErrorId};
use crate::msg;
use crate::paths;

use crate::support::StatusValue;

use crate::commands::status::global::{GlobalStatus, push};

use super::is_writable_dir;

/// `~/.sbxm`そのものの読み書き可否。
pub fn check_state_directory(location: &ConfigLocation, status: &mut GlobalStatus) {
    let dir = location.dir();
    let value = match std::fs::symlink_metadata(&dir) {
        // 未作成であること自体はerrorではない。最初の登録が作る。
        Err(_) => StatusValue::Missing,
        Ok(metadata) if !metadata.is_dir() => {
            status.diagnostics.push(
                Diagnostic::new(
                    ErrorId::GlobalStateUnusable,
                    msg!("error-global-state-unusable"),
                )
                .fact(Fact::path(&paths::display(&dir)))
                .fact(Fact::reason(msg!("cause-not-a-directory"))),
            );
            StatusValue::Error
        }
        Ok(_) if !is_writable_dir(&dir) => {
            status.diagnostics.push(
                Diagnostic::new(
                    ErrorId::GlobalStateUnusable,
                    msg!("error-global-state-unusable"),
                )
                .fact(Fact::path(&paths::display(&dir)))
                .fact(Fact::reason(msg!("cause-not-writable"))),
            );
            StatusValue::Error
        }
        Ok(_) => StatusValue::Ready,
    };
    push(status, "status-item-state-directory", value);
}
