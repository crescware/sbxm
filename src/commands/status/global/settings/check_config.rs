use crate::config::{self, ConfigLocation, ConfigState, GlobalConfig};

use crate::support::StatusValue;

use crate::commands::status::global::{GlobalStatus, push};

/// 任意のglobal config。不在は`defaults`として正常に扱う。
///
/// 読めた設定は、後続の診断が同じfileをもう一度読まずに済むよう返す。読めなければ
/// `None`とし、その事実はここで一度だけ報告する。
pub fn check_config(location: &ConfigLocation, status: &mut GlobalStatus) -> Option<GlobalConfig> {
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
