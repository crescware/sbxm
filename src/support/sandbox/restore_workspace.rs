use std::path::Path;

use crate::boundary::host::protocol::SandboxEntry;
use crate::diagnostics::Result;
use crate::msg;
use crate::project::SandboxName;

use super::{ReadySandbox, restore_found::restore_found, unusable, workspace_exists};

/// 記録がある既存Sandboxのidentityを検証し、消えていたworkspace directoryを復元する。
///
/// `ensure`と違い、対応するSandboxが無い場合に新規作成はしない。新規作成には利用する
/// Templateが要るが、この関数を呼ぶ側は「登録済みのはずのSandboxを開く」場面であり、
/// 空のTemplateを`ensure`へ渡して作成分岐を通す理由にしない。
///
/// Sandboxは`entries`から探し、hostへは問い合わせない。呼び出し側は、project lockの後に
/// 読んだ一覧か、観測に使った一覧をすでに持っている。同じlockの下でSandboxのrecordを
/// 変えるのは自分の作成と起動だけであり、その前に読んだ一覧はここでも正しい。
pub fn restore_workspace(
    entries: &[SandboxEntry],
    sandbox: &SandboxName,
    workspace_root: &Path,
) -> Result<ReadySandbox> {
    // 作る前に観測する。作ってから見ると、消えていたという事実はもう残っていない。
    // symlinkなど不正なworkspace pathは、ここで拒否する。
    let present = workspace_exists(workspace_root, sandbox)?;
    let Some(entry) = entries
        .iter()
        .find(|entry| entry.name == sandbox.as_str())
        .cloned()
    else {
        return Err(unusable(sandbox.as_str(), msg!("cause-sandbox-absent")));
    };
    restore_found(sandbox, workspace_root, entry, present)
}
