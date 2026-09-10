use std::path::Path;

use crate::boundary::host::protocol::SandboxEntry;
use crate::diagnostics::Result;
use crate::paths::{self, PRIVATE_DIR_MODE, PathScope};
use crate::project::SandboxName;

use super::{ReadySandbox, verify, workspace_path};

/// `workspace_exists`と`find`を呼び出し側が既に済ませたうえで復元する。`ensure`は
/// 自分で両方を呼んだあとここへ委ねることで、同じ一覧照会を二重に発行しない。
pub(super) fn restore_found(
    sandbox: &SandboxName,
    workspace_root: &Path,
    entry: SandboxEntry,
    present: bool,
) -> Result<ReadySandbox> {
    let workspace = workspace_path(workspace_root, sandbox);
    // 対応関係を確認できていないexpected pathを、確認より前に作らない。同名の
    // 既存Sandboxが別workspaceを指している場合、mismatchで失敗させ、hostには
    // まだ触れない。
    verify(&entry, sandbox, &workspace)?;
    // rootを別accountが所有していると、その下のworkspaceを入れ替えられる。
    paths::ensure_private_dir(workspace_root, PRIVATE_DIR_MODE, PathScope::ProjectPath)?;
    paths::ensure_private_dir(&workspace, PRIVATE_DIR_MODE, PathScope::ProjectPath)?;
    Ok(ReadySandbox {
        name: entry.name,
        workspace,
        state: entry.state,
        created: false,
        // recordが在るのにmount元が無かった場合だけ、作り直しとして扱う。
        workspace_restored: !present,
    })
}
