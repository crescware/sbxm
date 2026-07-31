use std::path::{Path, PathBuf};

/// Sandbox内の`agent` homeへ配置するhost file。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostFileSource(PathBuf);

impl HostFileSource {
    pub fn new(value: &str) -> std::result::Result<HostFileSource, &'static str> {
        let path = PathBuf::from(value);
        if value.is_empty() {
            return Err("the source is empty");
        }
        if !path.is_absolute() {
            return Err("the source is not an absolute path");
        }
        Ok(HostFileSource(path))
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }
}
