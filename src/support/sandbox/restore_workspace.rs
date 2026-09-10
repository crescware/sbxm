use std::path::Path;

use crate::diagnostics::Result;
use crate::msg;
use crate::project::SandboxName;

use crate::boundary::host::HostEnvironment;

use super::{ReadySandbox, find, restore_found::restore_found, unusable, workspace_exists};

/// 記録がある既存Sandboxのidentityを検証し、消えていたworkspace directoryを復元する。
///
/// `ensure`と違い、対応するSandboxが無い場合に新規作成はしない。新規作成には利用する
/// Templateが要るが、この関数を呼ぶ側は「登録済みのはずのSandboxを開く」場面であり、
/// 空のTemplateを`ensure`へ渡して作成分岐を通す理由にしない。
pub fn restore_workspace(
    host: &dyn HostEnvironment,
    sandbox: &SandboxName,
    workspace_root: &Path,
) -> Result<ReadySandbox> {
    // 作る前に観測する。作ってから見ると、消えていたという事実はもう残っていない。
    // symlinkなど不正なworkspace pathは、hostへ一度も問い合わせずここで拒否する。
    let present = workspace_exists(workspace_root, sandbox)?;
    let Some(entry) = find(host, sandbox)? else {
        return Err(unusable(sandbox.as_str(), msg!("cause-sandbox-absent")));
    };
    restore_found(sandbox, workspace_root, entry, present)
}
