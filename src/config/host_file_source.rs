use std::path::{Path, PathBuf};

use crate::diagnostics::Msg;
use crate::msg;

/// Sandbox内の`agent` homeへ配置するhost file。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostFileSource(PathBuf);

impl HostFileSource {
    /// 受け付けられない理由は、報告する側が翻訳できるようmessageで返す。
    pub fn new(value: &str) -> std::result::Result<HostFileSource, Msg> {
        let path = PathBuf::from(value);
        if value.is_empty() {
            return Err(msg!("cause-value-empty"));
        }
        if !path.is_absolute() {
            return Err(msg!("cause-not-absolute"));
        }
        Ok(HostFileSource(path))
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }
}
