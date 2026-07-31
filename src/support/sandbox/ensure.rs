use std::path::Path;

use crate::command::{CommandSpec, EnvPolicy, HostEnvironment, TimeoutClass};
use crate::diagnostics::Result;
use crate::msg;
use crate::paths::{self, PRIVATE_DIR_MODE, PathScope};
use crate::project::SandboxName;

use crate::design::ProgressSink;
use crate::support::template::LoadedTemplate;

use super::{AGENT_KIT, ReadySandbox, find, unusable, verify, workspace_path};

/// Sandboxを用意する。
///
/// 呼び出し側はdaemonの安全性を確認した区間で呼ぶ。
pub fn ensure(
    host: &dyn HostEnvironment,
    sandbox: &SandboxName,
    template: &LoadedTemplate,
    workspace_root: &Path,
    progress: &mut dyn ProgressSink,
) -> Result<ReadySandbox> {
    // rootを別accountが所有していると、その下のworkspaceを入れ替えられる。
    paths::ensure_private_dir(workspace_root, PRIVATE_DIR_MODE, PathScope::ProjectPath)?;
    let workspace = workspace_path(workspace_root, sandbox);
    paths::ensure_private_dir(&workspace, PRIVATE_DIR_MODE, PathScope::ProjectPath)?;

    if let Some(entry) = find(host, sandbox)? {
        verify(&entry, sandbox, &workspace)?;
        return Ok(ReadySandbox {
            name: entry.name,
            workspace,
            state: entry.state,
            created: false,
        });
    }

    progress.step(msg!("progress-creating-sandbox"));
    let spec = CommandSpec::passthrough(
        "sbx",
        &[
            "create",
            "--name",
            sandbox.as_str(),
            "--template",
            &template.name,
            AGENT_KIT,
            &paths::display(&workspace),
        ],
    )
    .env(EnvPolicy::InheritWithoutSshAgent)
    .timeout(TimeoutClass::SandboxLifecycle);
    host.run(&spec)?.require_success()?;

    let Some(entry) = find(host, sandbox)? else {
        return Err(unusable(
            sandbox.as_str(),
            "the sandbox is absent right after it was created",
        ));
    };
    verify(&entry, sandbox, &workspace)?;

    Ok(ReadySandbox {
        name: entry.name,
        workspace,
        state: entry.state,
        created: true,
    })
}
