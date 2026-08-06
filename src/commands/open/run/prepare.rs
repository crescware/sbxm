use std::path::Path;

use crate::command::{CommandSpec, HostEnvironment};
use crate::config::ConfigLocation;
use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::metadata::{MAX_WORKTREE_INDEX, ProjectMetadata, last_worktree_index};
use crate::msg;
use crate::project::{ProjectId, SandboxLayout};

use crate::design::ProgressSink;
use crate::support::inventory::{self, Poll, ProjectState};
use crate::support::select::{self, ProjectPrompt};
use crate::support::{daemon, generation, sandbox, worktree};

use super::{ClampedIndex, Prepared};

/// `SSHへ引き渡せる状態までSandboxを整える`。
///
/// 1. 対象を引数またはpromptで解決し、必要なら案件とindexを1画面で選ぶ
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
    let locked = candidate.lock()?;
    // project lockを保持している間にshared session leaseを取る。lock順序を
    // project lock→session leaseに固定し、`locked`が外れたあともこのleaseは
    // `Prepared`が保持し続けるため、SSH sessionの生存中は通常rebuild/destroyの
    // exclusive session leaseと排他し続ける。
    let session_lease = locked.acquire_shared_session_lease()?;
    let (index, clamped_worktree_index) = if interactive_index {
        clamp_to_metadata(index, &locked.metadata)
    } else {
        (index, None)
    };

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
        clamped_worktree_index,
        worktrees,
        _session_lease: session_lease,
    })
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
