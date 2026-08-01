use super::*;

impl ConfigLocation {
    /// 指定したhome directoryから組み立てる。
    pub fn from_home(home: PathBuf) -> ConfigLocation {
        ConfigLocation { home }
    }
}
