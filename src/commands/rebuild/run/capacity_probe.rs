use crate::boundary::host::{HostEnvironment, TimeoutClass};
use crate::design::{Fact, ProgressSink, Remediation};
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::hash::{sha256_hex, short_hex};
use crate::msg;
use crate::paths;
use crate::project::{ProjectId, SandboxName};
use crate::support::{daemon, inventory, sandbox, template::LoadedTemplate};

/// 新Templateの空き容量を、旧Sandboxを残したまま一時VMで確認する。
pub(super) fn capacity_probe(
    host: &dyn HostEnvironment,
    workspace_root: &std::path::Path,
    template: &LoadedTemplate,
    poll: inventory::Poll,
    progress: &mut dyn ProgressSink,
    check: impl FnOnce(&str) -> Result<()>,
) -> Result<()> {
    paths::ensure_private_dir(
        workspace_root,
        paths::PRIVATE_DIR_MODE,
        paths::PathScope::ProjectPath,
    )?;
    let directory = tempfile::Builder::new()
        .prefix(".capacity-check-")
        .tempdir_in(workspace_root)
        .map_err(|error| paths::atomic_write_failed(workspace_root, &error.to_string()))?;
    let nonce = sha256_hex(paths::display(directory.path()).as_bytes());
    let id = ProjectId::parse(&format!("capacity-check/{}", short_hex(&nonce)))?;
    let name = SandboxName::derive(&id.canonical());
    let workspace = sandbox::workspace_path(directory.path(), &name);
    paths::ensure_private_dir(
        &workspace,
        paths::PRIVATE_DIR_MODE,
        paths::PathScope::ProjectPath,
    )?;
    progress.step(msg!("progress-rebuild-capacity-probe"));
    let create = sandbox::relayed(&[
        "create",
        "--name",
        name.as_str(),
        "--template",
        &template.name,
        sandbox::AGENT_KIT,
        &paths::display(&workspace),
    ])
    .timeout(TimeoutClass::SandboxLifecycle);
    let result = host
        .run_with_terminal(&create, progress)
        .and_then(crate::boundary::host::CommandOutcome::require_success)
        .and_then(|_| check(name.as_str()))
        .map_err(|error| match error {
            Error::Diagnostics(diagnostics) => Error::Diagnostics(
                diagnostics
                    .into_iter()
                    .map(|diagnostic| diagnostic.fact(Fact::sandbox(name.as_str())))
                    .collect(),
            ),
            Error::Canceled => Error::Canceled,
        });
    // createが途中で失敗しても、この実行のprivate workspaceと一致するVMだけを片付ける。
    let cleanup = cleanup(host, &name, &workspace, poll);
    match (result, cleanup) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) => Err(error),
        (result, Err(cleanup)) => {
            // VMが残った場合はmount点も保持し、名前と片付ける手順を必ず知らせる。
            let workspace_root = directory.keep();
            let mut diagnostics = result
                .err()
                .map_or_else(Vec::new, |error| error.diagnostics().to_vec());
            diagnostics.extend_from_slice(cleanup.diagnostics());
            diagnostics.push(
                Diagnostic::new(
                    ErrorId::RebuildCapacityUnconfirmed,
                    msg!("error-rebuild-capacity-unconfirmed"),
                )
                .fact(Fact::sandbox(name.as_str()))
                .fact(Fact::path(&paths::display(&workspace_root)))
                .remediation(
                    Remediation::text(msg!("remediation-capacity-probe-left"))
                        .try_run(format!("sbx rm {}", name.as_str())),
                ),
            );
            Err(Error::Diagnostics(diagnostics))
        }
    }
}

fn cleanup(
    host: &dyn HostEnvironment,
    name: &SandboxName,
    workspace: &std::path::Path,
    poll: inventory::Poll,
) -> Result<()> {
    let entries = daemon::list(host)?;
    let Some(entry) = inventory::single(&entries, name.as_str())? else {
        return Ok(());
    };
    // 保存先を確認できない同名VMは、この実行のものと推測して削除しない。
    sandbox::verify_identity(entry, name, workspace.parent().unwrap_or(workspace))?;
    sandbox::remove_preserving_secrets(host, name, false)?;
    inventory::wait_until_absent(host, name, poll)
}
