use std::path::Path;

use crate::command::{HostEnvironment, TimeoutClass};
use crate::diagnostics::Result;
use crate::msg;
use crate::paths::{self, PRIVATE_DIR_MODE, PathScope};
use crate::project::SandboxName;

use crate::design::ProgressSink;
use crate::support::template::LoadedTemplate;

use super::{
    AGENT_KIT, ReadySandbox, find, relayed, unusable, verify, workspace_exists, workspace_path,
};

/// Sandboxを用意する。
///
/// 呼び出し側はdaemonの安全性を確認した区間で呼ぶ。
///
/// 既にrecordがある案件でworkspace directoryが消えている場合は、この関数がそれを
/// 作り直す。作り直した事実は`ReadySandbox`へ載せ、呼び出し側が成功のなかへ黙って
/// 混ぜずに告げられるようにする。
pub fn ensure(
    host: &dyn HostEnvironment,
    sandbox: &SandboxName,
    template: &LoadedTemplate,
    workspace_root: &Path,
    progress: &mut dyn ProgressSink,
) -> Result<ReadySandbox> {
    let workspace = workspace_path(workspace_root, sandbox);
    // 作る前に観測する。作ってから見ると、消えていたという事実はもう残っていない。
    let present = workspace_exists(workspace_root, sandbox)?;
    // rootを別accountが所有していると、その下のworkspaceを入れ替えられる。
    paths::ensure_private_dir(workspace_root, PRIVATE_DIR_MODE, PathScope::ProjectPath)?;
    paths::ensure_private_dir(&workspace, PRIVATE_DIR_MODE, PathScope::ProjectPath)?;

    if let Some(entry) = find(host, sandbox)? {
        verify(&entry, sandbox, &workspace)?;
        return Ok(ReadySandbox {
            name: entry.name,
            workspace,
            state: entry.state,
            created: false,
            // recordが在るのにmount元が無かった場合だけ、作り直しとして扱う。
            workspace_restored: !present,
        });
    }

    progress.step(msg!("progress-creating-sandbox"));
    let command = relayed(&[
        "create",
        "--name",
        sandbox.as_str(),
        "--template",
        &template.name,
        AGENT_KIT,
        &paths::display(&workspace),
    ])
    .timeout(TimeoutClass::SandboxLifecycle);
    host.run_with_terminal(&command, progress)?
        .require_success()?;

    let Some(entry) = find(host, sandbox)? else {
        return Err(unusable(
            sandbox.as_str(),
            msg!("cause-sandbox-absent-after-create"),
        ));
    };
    verify(&entry, sandbox, &workspace)?;

    Ok(ReadySandbox {
        name: entry.name,
        workspace,
        state: entry.state,
        created: true,
        // 新しく作るSandboxのmount点であり、消えたものの作り直しではない。
        workspace_restored: false,
    })
}
