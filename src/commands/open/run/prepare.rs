use std::path::Path;

use crate::command::{CommandSpec, HostEnvironment};
use crate::config::ConfigLocation;
use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::metadata::ProjectMetadata;
use crate::msg;
use crate::project::{ProjectId, SandboxLayout};

use crate::design::ProgressSink;
use crate::support::inventory::{self, Poll, ProjectState};
use crate::support::select::{self, ProjectPrompt};
use crate::support::{daemon, generation, sandbox, worktree};

use super::Prepared;

/// `SSHへ引き渡せる状態までSandboxを整える`。
///
/// 1. 対象を引数またはpromptで解決する
/// 2. project lockを取得する
/// 3. Docker Engineへの疎通を確認する
/// 4. 1回の一覧取得からSandbox identityとstateを検証する
/// 5. runningでなければ起動して待つ
/// 6. hostのSSH `Agentが届かないことをSandboxの中から確認する`
/// 7. managed worktreeをmetadataとGitから検証する
///
/// lockはこの関数のあいだだけ保持する。SSH sessionそのものはsbxmのmutationではなく、
/// 接続中に別terminalの`stop`が待たされる状態を作らない。
#[allow(clippy::too_many_arguments)]
pub fn prepare(
    location: &ConfigLocation,
    requested: Option<&ProjectId>,
    index: Option<u32>,
    host: &dyn HostEnvironment,
    prompt: &mut dyn ProjectPrompt,
    workspace_root: &Path,
    poll: Poll,
    progress: &mut dyn ProgressSink,
) -> Result<Prepared> {
    // 対象が決まる前にhostの状態へ触れない。
    let locked = select::one(location, requested, &msg!("select-open-heading"), prompt)?.lock()?;

    let metadata = &locked.metadata;
    let name = metadata.sandbox_name();
    generation::require_no_rebuild(metadata)?;

    require_docker(host)?;

    let entries = daemon::list(host)?;
    match inventory::state_of(&entries, metadata, workspace_root)? {
        ProjectState::Running => {}
        ProjectState::Stopped => inventory::start(host, name.as_str(), progress)?,
        ProjectState::NotCreated => return Err(inventory::not_created(metadata, name.as_str())),
    }
    inventory::wait_until_running(host, metadata, workspace_root, poll)?;

    // 接続する前に、hostのSSH Agentが届かないことを中から確かめる。
    sandbox::require_credentials_isolated(host, name.as_str())?;

    let layout = SandboxLayout::new(metadata.canonical_id());
    let worktrees = verify_worktrees(host, name.as_str(), &layout, metadata)?;
    let (working_directory, missing_worktree_index) = working_directory(&layout, &worktrees, index);

    Ok(Prepared {
        project: metadata.display_id(),
        sandbox: name.as_str().to_string(),
        ssh_host: format!("{name}.sbx"),
        working_directory,
        missing_worktree_index,
        worktrees,
    })
}

/// indexなし、または見つからないindexはrepository rootへ接続する。
fn working_directory(
    layout: &SandboxLayout,
    worktrees: &[String],
    index: Option<u32>,
) -> (String, Option<u32>) {
    let Some(requested) = index else {
        return (layout.bare_root(), None);
    };
    let Some(index) = usize::try_from(requested).ok() else {
        return (layout.bare_root(), Some(requested));
    };
    match worktrees.get(index) {
        Some(path) => (path.clone(), None),
        None => (layout.bare_root(), Some(requested)),
    }
}

/// Docker Engineへ疎通できることを確認する。
fn require_docker(host: &dyn HostEnvironment) -> Result<()> {
    let spec = CommandSpec::probe("docker", &["version", "--format", "{{.Server.Version}}"]);
    let outcome = host.run(&spec)?;
    if outcome.success() && !outcome.stdout_text().trim().is_empty() {
        return Ok(());
    }
    Err(Error::single(
        Diagnostic::new(ErrorId::DockerUnreachable, msg!("error-docker-unreachable"))
            .fact(Fact::reason(msg!("cause-server-version-unreadable")))
            .remediation(msg!("remediation-start-docker")),
    ))
}

/// metadataが宣言するmanaged worktreeが、Sandbox内のGitに揃っていることを確認する。
fn verify_worktrees(
    host: &dyn HostEnvironment,
    name: &str,
    layout: &SandboxLayout,
    metadata: &ProjectMetadata,
) -> Result<Vec<String>> {
    let bare_root = layout.bare_root();
    let listed: Vec<String> = worktree::list(host, name, layout)?
        .iter()
        .filter_map(|entry| entry.relative_to(&bare_root))
        .collect();

    let names = layout.worktree_names(metadata.provisioning.requested_worktrees);
    let mut present = Vec::with_capacity(names.len());
    for name in names {
        if !listed.contains(&name) {
            let path = format!("{bare_root}/{name}");
            return Err(Error::single(
                Diagnostic::new(
                    ErrorId::SandboxRepositoryUnusable,
                    msg!("error-sandbox-repository-unusable"),
                )
                .fact(Fact::path(&path))
                .fact(Fact::reason(msg!("cause-managed-worktree-absent")))
                .remediation(msg!("remediation-sandbox-repository-unusable", path = path)),
            ));
        }
        present.push(format!("{bare_root}/{name}"));
    }
    Ok(present)
}
