use std::fs;
use std::path::Path;

use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;
use crate::paths::{self, PRIVATE_DIR_MODE, PathScope};
use crate::project::SandboxName;
use crate::support::Observed;

use super::workspace_path;

/// 中立workspace directoryを、安全性まで確かめて観測する。
///
/// 存在するだけでは`Ready`側へ丸めない。symlink、他アカウント所有、group/otherへの
/// permissionは`Unsafe`として拒否する。observationの型が持たない不確実さ（読み取り
/// 自体の失敗）は`Unobservable`として同じく拒否する。Sandboxが存在しないorphanな
/// workspaceは、utility以外の中身を持たないことまで確かめる。
///
/// 拒否は`Ready`にも`Incomplete`にも丸めないため、呼び出し側へ`Err`のまま返す。
pub fn observe_workspace(
    workspace_root: &Path,
    sandbox: &SandboxName,
    sandbox_present: bool,
) -> Result<Observed> {
    let workspace = workspace_path(workspace_root, sandbox);
    match fs::symlink_metadata(&workspace) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Observed::Missing),
        Err(error) => Err(PathScope::ProjectPath.unreadable_error(&workspace, &error.to_string())),
        Ok(_) => {
            paths::require_private_directory(&workspace, PRIVATE_DIR_MODE, PathScope::ProjectPath)?;
            if !sandbox_present {
                require_empty(&workspace)?;
            }
            Ok(Observed::Matching)
        }
    }
}

/// 対応するSandboxがない状態で残っているworkspaceは、sbxmが作った空のmount点だけを
/// 安全な成果物として認める。中身がある場合、それがどこから来たかを確認できない。
fn require_empty(workspace: &Path) -> Result<()> {
    let mut entries = fs::read_dir(workspace)
        .map_err(|error| PathScope::ProjectPath.unreadable_error(workspace, &error.to_string()))?;
    if entries.next().is_none() {
        return Ok(());
    }
    Err(Error::single(
        Diagnostic::new(
            ErrorId::SandboxWorkspaceNotEmpty,
            msg!("error-sandbox-workspace-not-empty"),
        )
        .fact(Fact::path(&paths::display(workspace)))
        .fact(Fact::reason(msg!("cause-sandbox-workspace-not-empty"))),
    ))
}

#[cfg(test)]
#[path = "observe_workspace_test.rs"]
mod observe_workspace_test;
