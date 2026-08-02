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
    /// 現在の利用者のhome directoryから構築する。
    pub fn discover() -> Result<ConfigLocation> {
        ConfigLocation::from_home_directory(dirs::home_dir())
    }

    /// home directoryの読み取り結果から構築する。
    ///
    /// processのenvironmentを読むのは`discover`だけとし、読めなかったときの判断は値を
    /// 受け取る側へ置く。環境変数を書き換えずに、その拒否を確かめられる。
    fn from_home_directory(home: Option<PathBuf>) -> Result<ConfigLocation> {
        // home directoryが分からない時点でpathは1つも組み立てられない。示せる事実は
        // 読めなかった理由だけである。
        let home = home.ok_or_else(|| {
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

#[cfg(test)]
#[path = "config_location_test.rs"]
mod config_location_test;
