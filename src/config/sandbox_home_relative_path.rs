use std::path::{Component, Path, PathBuf};

use crate::diagnostics::Msg;
use crate::msg;

/// Sandbox内の`agent` homeからの相対path。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxHomeRelativePath(PathBuf);

impl SandboxHomeRelativePath {
    /// 受け付けられない理由は、報告する側が翻訳できるようmessageで返す。
    pub fn new(value: &str) -> std::result::Result<SandboxHomeRelativePath, Msg> {
        let path = PathBuf::from(value);
        if value.is_empty() {
            return Err(msg!("cause-value-empty"));
        }
        if path.is_absolute() {
            return Err(msg!("cause-unexpectedly-absolute"));
        }
        if path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
        {
            return Err(msg!("cause-has-parent-directory-component"));
        }
        Ok(SandboxHomeRelativePath(path))
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }
}
