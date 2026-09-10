use std::path::Path;

use crate::boundary::host::{HostEnvironment, TimeoutClass};
use crate::config::{ConfigLocation, GlobalConfig};
use crate::design::{Fact, Warning};
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::metadata::{MAX_WORKTREE_INDEX, ProjectMetadata, last_worktree_index};
use crate::msg;
use crate::project::{ProjectId, SandboxLayout};

use crate::design::ProgressSink;
use crate::support::inventory::{self, Poll, ProjectState};
use crate::support::select::{self, ProjectPrompt};
use crate::support::{daemon, disk, docker, generation, provisioning, sandbox, worktree};

use super::{ClampedIndex, Prepared};

/// `SSHへ引き渡せる状態までSandboxを整える`。
///
/// 1. 対象を引数またはpromptで解決し、必要なら案件とindexを1画面で選ぶ
/// 2. project lockを取得する
/// 3. 外側の状態を観測し、中断した初回構築があれば同じopenで再開する
/// 4. 必要な準備中はexclusive session leaseを保持する
/// 5. runningでなければworkspaceを検証・復元して起動する
/// 6. 不足する初回構築またはmanaged worktreeを完成させる
/// 7. exclusive leaseをshared session leaseへ引き継ぐ
/// 8. hostのSSH `Agentが届かないことをSandboxの中から確認する`
/// 9. managed worktreeをmetadataとGitから検証する
///
/// lockはこの関数のあいだだけ保持する。準備mutationはexclusive session lease、接続は
/// shared session leaseで守り、project lockを保持したまま両者を引き継ぐ。
#[allow(clippy::too_many_arguments)]
pub fn prepare(
    location: &ConfigLocation,
    config: &GlobalConfig,
    requested: Option<&ProjectId>,
    index: Option<u32>,
    host: &dyn HostEnvironment,
    prompt: &mut dyn ProjectPrompt,
    workspace_root: &Path,
    poll: Poll,
    progress: &mut dyn ProgressSink,
) -> Result<Prepared> {
    let interactive_index = requested.is_none() && index.is_none();
    // 対象が決まる前にhostの状態へ触れない。metadataもprompt表示前には待たないため、
    // interactiveなindexは設定上限相当の楽観的な値まで受け付ける。promptの裏で計算が終われば
    // 表示中の最大値へ反映し、最後はlock済みmetadataでclampする。clampした事実は
    // `Prepared`へ載せ、接続前に見せる。
    let (candidate, index) = if interactive_index {
        let (candidate, index) = select::open(
            location,
            &msg!("select-open-heading"),
            prompt,
            MAX_WORKTREE_INDEX,
        )?;
        (candidate, Some(index))
    } else {
        (
            select::one(location, requested, &msg!("select-open-heading"), prompt)?,
            index,
        )
    };
    let mut locked = candidate.lock()?;
    let (index, clamped_worktree_index) = if interactive_index {
        clamp_to_metadata(index, &locked.metadata)
    } else {
        (index, None)
    };

    generation::require_no_rebuild(&locked.metadata)?;
    docker::require_reachable(host)?;

    let (provisioned, warnings) =
        prepare_runtime(&mut locked, config, host, workspace_root, poll, progress)?;

    // 準備と起動が完了した状態をSSH sessionとして保護する。
    let session_lease = locked.acquire_shared_session_lease()?;

    let metadata = &locked.metadata;
    let name = metadata.sandbox_name();
    let layout = SandboxLayout::new(metadata.canonical_id());

    // 接続する前に、hostのSSH Agentが届かないことを中から確かめる。
    sandbox::require_credentials_isolated(host, name.as_str())?;

    let worktrees = verify_worktrees(host, name.as_str(), &layout, metadata)?;
    let (working_directory, missing_worktree_index) = working_directory(&layout, &worktrees, index);

    // ここまで来た時点でSandboxは必ずrunningである。この観測のために追加で
    // 起動しない。値または理由はSSHへterminalを渡す前に1回だけ示す。
    let disk = disk::observe(
        host,
        name.as_str(),
        Some(ProjectState::Running),
        TimeoutClass::Probe,
    );

    Ok(Prepared {
        project: metadata.display_id(),
        sandbox: name.as_str().to_string(),
        ssh_host: format!("{name}.sbx"),
        working_directory,
        missing_worktree_index,
        clamped_worktree_index,
        worktrees,
        disk,
        provisioned,
        warnings,
        _session_lease: session_lease,
    })
}

fn prepare_runtime(
    locked: &mut crate::support::select::Locked,
    config: &GlobalConfig,
    host: &dyn HostEnvironment,
    workspace_root: &Path,
    poll: Poll,
    progress: &mut dyn ProgressSink,
) -> Result<(Option<provisioning::ProvisioningOutput>, Vec<Warning>)> {
    let entries = daemon::list(host)?;
    let state = inventory::state_of(&entries, &locked.metadata, workspace_root)?;
    let mut warnings = Vec::new();
    if state == ProjectState::Stopped
        && let Some(warning) = restore_workspace(host, &locked.metadata, workspace_root)?
    {
        warnings.push(warning);
    }
    let needs_initial =
        locked.metadata.initial_provisioning.is_some() || state == ProjectState::NotCreated;
    let provisioned = if needs_initial {
        // 準備のmutationは既存SSH sessionと共存させない。完了後、project lockを保持した
        // ままexclusiveを解放してsharedへ移るため、lifecycle操作が間へ入らない。
        let exclusive = locked.acquire_exclusive_session_lease()?;
        if state == ProjectState::Stopped {
            inventory::start(host, &locked.metadata, workspace_root, progress)?;
            inventory::wait_until_running(host, &locked.metadata, workspace_root, poll)?;
        }
        let output = provisioning::ensure_initial(locked, config, host, workspace_root, progress)?;
        drop(exclusive);
        Some(output)
    } else {
        if state == ProjectState::Stopped {
            inventory::start(host, &locked.metadata, workspace_root, progress)?;
        }
        None
    };
    inventory::wait_until_running(host, &locked.metadata, workspace_root, poll)?;

    // intentは無いがSandboxは在る案件で、成果物の一部が欠けている場合だけ、観測できた
    // 不足工程をこのopenで補う。`status`はこの状態にも`sbxm open`を案内するため、
    // ここで直せない欠落を残さない。
    let provisioned = if provisioned.is_none() {
        complete_missing_interior(locked, config, host, workspace_root, progress)?
    } else {
        provisioned
    };
    Ok((provisioned, warnings))
}

/// intentが無い案件のIncompleteを、記録済みbaselineから観測できた不足工程だけ補う。
///
/// image/archive/Template/Sandbox作成には触れない。それらが欠けている場合は
/// `InitialRoute::decide`が`Build`へ送り、ここへは来ない。
fn complete_missing_interior(
    locked: &mut crate::support::select::Locked,
    config: &GlobalConfig,
    host: &dyn HostEnvironment,
    workspace_root: &Path,
    progress: &mut dyn ProgressSink,
) -> Result<Option<provisioning::ProvisioningOutput>> {
    let observation = provisioning::observe(
        host,
        &locked.paths,
        config,
        &locked.metadata,
        workspace_root,
    )?;
    observation.require_safe()?;
    if observation.state != provisioning::ProvisioningState::Incomplete {
        return Ok(None);
    }
    let exclusive = locked.acquire_exclusive_session_lease()?;
    let files = locked.metadata.declared_files.clone().unwrap_or_default();
    let inputs = provisioning::ProvisioningInputs::from_recorded_files(&locked.paths, &files)?;
    let output = provisioning::provision_interior(locked, &inputs, host, progress, Vec::new())?;
    drop(exclusive);

    let completed = provisioning::observe(
        host,
        &locked.paths,
        config,
        &locked.metadata,
        workspace_root,
    )?;
    completed.require_safe()?;
    // workspaceが確認できないままだと、内部は補ったあとも観測不能のまま残る。それは
    // この経路が直せなかった欠落ではなく、動いているSandboxの中を推測で読まなかった
    // だけである。ここで拒否せず、接続はそのまま進める。
    if !completed.is_complete() && !completed.interior_is_unobservable() {
        return Err(provisioning::require_open(
            &locked.metadata,
            provisioning::ProvisioningState::Incomplete,
        ));
    }
    Ok(Some(output))
}

fn restore_workspace(
    host: &dyn HostEnvironment,
    metadata: &ProjectMetadata,
    workspace_root: &Path,
) -> Result<Option<Warning>> {
    let ready = sandbox::restore_workspace(host, &metadata.sandbox_name(), workspace_root)?;
    Ok(ready.workspace_restored.then(|| {
        Warning::text(msg!("warning-workspace-restored", sandbox = ready.name))
            .fact(Fact::path(&crate::paths::display(&ready.workspace)))
            .explain(msg!("guidance-workspace-restored"))
    }))
}

/// promptの楽観的な上限で確定したindexを、lock済みmetadataの範囲へ収める。
///
/// 収めた場合は、その内訳も返す。呼び出し側は接続前にそれを見せる。
fn clamp_to_metadata(
    index: Option<u32>,
    metadata: &ProjectMetadata,
) -> (Option<u32>, Option<ClampedIndex>) {
    let maximum = last_worktree_index(metadata.provisioning.requested_worktrees);
    let Some(requested) = index else {
        return (None, None);
    };
    if requested <= maximum {
        return (Some(requested), None);
    }
    (
        Some(maximum),
        Some(ClampedIndex {
            requested,
            opened: maximum,
        }),
    )
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
