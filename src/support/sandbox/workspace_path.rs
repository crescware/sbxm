use std::path::{Path, PathBuf};

use crate::project::SandboxName;

/// `<workspace-root>/<sandbox-name>`
pub fn workspace_path(root: &Path, sandbox: &SandboxName) -> PathBuf {
    root.join(sandbox.as_str())
}
