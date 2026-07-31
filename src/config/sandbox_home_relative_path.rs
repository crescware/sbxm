use std::path::{Component, Path, PathBuf};

/// Sandbox内の`agent` homeからの相対path。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxHomeRelativePath(PathBuf);

impl SandboxHomeRelativePath {
    pub fn new(value: &str) -> std::result::Result<SandboxHomeRelativePath, &'static str> {
        let path = PathBuf::from(value);
        if value.is_empty() {
            return Err("the destination is empty");
        }
        if path.is_absolute() {
            return Err("the destination is an absolute path");
        }
        if path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
        {
            return Err("the destination contains a parent directory component");
        }
        Ok(SandboxHomeRelativePath(path))
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }
}
