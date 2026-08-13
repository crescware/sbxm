use std::path::Path;

use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::metadata::ProjectMetadata;
use crate::msg;
use crate::paths::{self, PRIVATE_DIR_MODE, PathScope};
use crate::project::SandboxName;

use crate::design::{Fact, Remediation};
use crate::support::sandbox;

/// 起動の前提として、中立workspace directoryがhostに在ることを確かめる。
///
/// runtimeは、recordがmount元として宣言するdirectoryが消えていると起動を拒む。実在は
/// runtime stateに含まれない別の事実であるため、起動を試す前にhostを実測する。実測を
/// 省いて起動させると、runtimeが返したHTTP statusだけが残り、何が欠けているかを
/// 利用者へ示せない。
///
/// 不在は専用の診断で拒否し、観測できない場合はその理由をそのまま返す。どちらの場合も
/// 起動しない。ここでdirectoryを作り直すことはしない。復旧は、対象と変更範囲を示せる
/// 構築commandの側で行う。
///
/// 実在は`directory`であることまでしか確かめない。cleanupのあとで別accountがroot・
/// workspaceのどちらかを予測可能な名前で作り直していると、そのaccountが用意した
/// directoryをmount元としてruntimeへ渡しかねない。実在の確認に続けて、両方が現在の
/// 利用者だけの所有・permissionであることまで確かめ、確認できなければ拒否する。
pub(super) fn require_workspace(
    metadata: &ProjectMetadata,
    sandbox_name: &SandboxName,
    workspace_root: &Path,
) -> Result<()> {
    if sandbox::workspace_exists(workspace_root, sandbox_name)? {
        paths::require_private_directory(workspace_root, PRIVATE_DIR_MODE, PathScope::ProjectPath)?;
        let workspace = sandbox::workspace_path(workspace_root, sandbox_name);
        paths::require_private_directory(&workspace, PRIVATE_DIR_MODE, PathScope::ProjectPath)?;
        return Ok(());
    }
    Err(Error::single(
        Diagnostic::new(
            ErrorId::SandboxWorkspaceMissing,
            msg!(
                "error-sandbox-workspace-missing",
                project = metadata.display_id(),
                sandbox = sandbox_name
            ),
        )
        .fact(Fact::path(&paths::display(&sandbox::workspace_path(
            workspace_root,
            sandbox_name,
        ))))
        // 中は空のmount点であり、案件の成果物はSandboxの中にある。作り直しは構築
        // commandの一部として、何を作ったかを示したうえで行う。
        .remediation(
            Remediation::text(msg!("remediation-sandbox-workspace-missing"))
                .try_run(format!("sbxm prepare {}", metadata.display_id())),
        ),
    ))
}
