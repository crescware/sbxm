use crate::design::Warning;

use super::GlobalConfig;

/// config loadの結果。
#[derive(Debug)]
pub enum ConfigState {
    /// configが存在しない。default設定として扱う。
    Missing,
    /// 有効なconfig。version 1では未知のtop-level keyをwarningとして返す。
    Valid {
        config: Box<GlobalConfig>,
        warnings: Vec<Warning>,
    },
}

impl ConfigState {
    /// 保存済みの設定、または不在を意味するdefault設定。
    pub fn settings(self) -> GlobalConfig {
        match self {
            ConfigState::Missing => GlobalConfig::default(),
            ConfigState::Valid { config, .. } => *config,
        }
    }
}
