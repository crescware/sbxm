use std::path::PathBuf;

use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
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
        // home directoryが分からない時点でpathは1つも組み立てられない。示せる事実は
        // 読めなかった理由だけである。
        let home = dirs::home_dir().ok_or_else(|| {
            Error::single(
                Diagnostic::new(ErrorId::ConfigUnreadable, msg!("error-config-unreadable"))
                    .fact(Fact::reason(msg!("cause-home-directory-unknown"))),
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
