use std::path::Path;

use crate::compatibility::SandboxEntry;
use crate::diagnostics::Result;
use crate::project::SandboxName;

use super::{verify, workspace_path};

/// 既存Sandboxが、この案件のものであることをread-onlyで確認する。
pub fn verify_identity(
    entry: &SandboxEntry,
    sandbox: &SandboxName,
    workspace_root: &Path,
) -> Result<()> {
    verify(entry, sandbox, &workspace_path(workspace_root, sandbox))
}
