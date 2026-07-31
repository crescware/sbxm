use std::path::Path;

use crate::compatibility::SandboxEntry;
use crate::diagnostics::Result;
use crate::paths::{self};
use crate::project::SandboxName;

use super::unusable;

/// 既存Sandboxが期待する構成であることを確認する。
///
/// 誰が作成したかは条件にしない。案件との対応は、canonical project IDから導出した
/// 名前と、その案件だけが使う中立Workspaceの実pathで判定する。由来Templateは一覧に
/// 現れないため、同一性の根拠にしない。
pub(super) fn verify(entry: &SandboxEntry, sandbox: &SandboxName, workspace: &Path) -> Result<()> {
    match &entry.workspace {
        Some(observed) => {
            let observed = paths::real_path(Path::new(observed));
            let expected = paths::real_path(workspace);
            if observed != expected {
                return Err(unusable(
                    sandbox.as_str(),
                    &format!(
                        "the sandbox works in {}, not in {}",
                        paths::display(&observed),
                        paths::display(&expected)
                    ),
                ));
            }
        }
        None => {
            return Err(unusable(
                sandbox.as_str(),
                "this Docker Sandboxes version does not report the workspace of a sandbox",
            ));
        }
    }

    Ok(())
}
