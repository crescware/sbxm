use std::path::PathBuf;

use crate::diagnostics::{Error, ErrorId, Result};
use crate::msg;

/// `~/.sbxm`配下の固定path。
///
/// home directoryを明示的に受け取り、processのenvironmentに依存しない。
#[derive(Debug, Clone)]
pub struct ConfigLocation {
    home: PathBuf,
}

impl ConfigLocation {
    #[cfg(test)]
    pub fn from_home(home: PathBuf) -> ConfigLocation {
        ConfigLocation { home }
    }

    /// 現在の利用者のhome directoryから構築する。
    pub fn discover() -> Result<ConfigLocation> {
        let home = dirs::home_dir().ok_or_else(|| {
            Error::new(
                ErrorId::ConfigUnreadable,
                msg!(
                    "error-config-unreadable",
                    path = "~",
                    detail = "the home directory could not be determined"
                ),
            )
        })?;
        Ok(ConfigLocation { home })
    }

    /// `~/.sbxm`
    pub fn dir(&self) -> PathBuf {
        self.home.join(".sbxm")
    }

    /// `~/.sbxm/config.yaml`
    pub fn config_file(&self) -> PathBuf {
        self.dir().join("config.yaml")
    }

    /// `~/.sbxm/registry.yaml`
    pub fn registry_file(&self) -> PathBuf {
        self.dir().join("registry.yaml")
    }

    /// `~/.sbxm/registry.lock`
    pub fn registry_lock(&self) -> PathBuf {
        self.dir().join("registry.lock")
    }
}
