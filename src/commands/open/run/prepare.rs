use std::path::Path;

use crate::boundary::host::{HostEnvironment, TimeoutClass};
use crate::config::{ConfigLocation, GlobalConfig};
use crate::design::Fact;
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
/// 3. 中断した初回構築が残っていないことを、hostへ触れる前にmetadataで確かめる
/// 4. Docker Engineへの疎通を確認する
/// 5. 1回の一覧取得からSandbox identityとstateを検証する
/// 6. Sandboxがまだ無ければ、共有境界で初回構築を完了させる
/// 7. runningでなければ起動して待つ
/// 8. hostのSSH `Agentが届かないことをSandboxの中から確認する`
/// 9. managed worktreeをmetadataとGitから検証する
///
/// lockはこの関数のあいだだけ保持する。初回構築もこのlockの下で進むため、構築中の
/// 排他はproject lockが担う。session leaseは構築中も接続中もsharedのままとし、lockが
/// 外れたあとも続くSSH sessionだけを、rebuild/destroy/repairのexclusive leaseと排他する。
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

    generation::require_no_rebuild(&locked.metadata)?;
    // 中断した初回構築を暗黙に再開しない。intentはmetadataだけで判定できるため、
    // repairへ渡す案件に対してはDockerにもsbxにも触れず、metadataも書き換えない。
    provisioning::require_no_initial_intent(&locked.metadata)?;

    docker::require_reachable(host)?;

    let entries = daemon::list(host)?;
    let provisioned = match inventory::state_of(&entries, &locked.metadata, workspace_root)? {
        // 既に動いているSandboxを起動し直さない。hostのworkspace directoryが消えていても、
        // 動いているmountが壊れているかどうかは観測しておらず、推測で接続を拒まない。
        ProjectState::Running => None,
        // 起動には中立workspace directoryの実在が要る。`start`が起動前に実測する。
        ProjectState::Stopped => {
            inventory::start(host, &locked.metadata, workspace_root, progress)?;
            None
        }
        // Sandboxをまだ持たない案件だけが初回構築の対象になる。停止中の完成済み案件は
        // この経路へ来ないため、中を観測できないことを理由にrepairへ送らない。
        ProjectState::NotCreated => Some(provisioning::ensure_initial(
            &mut locked,
            config,
            host,
            workspace_root,
            progress,
        )?),
    };
    inventory::wait_until_running(host, &locked.metadata, workspace_root, poll)?;

    let metadata = &locked.metadata;
    let name = metadata.sandbox_name();

    // 接続する前に、hostのSSH Agentが届かないことを中から確かめる。
    sandbox::require_credentials_isolated(host, name.as_str())?;

    let layout = SandboxLayout::new(metadata.canonical_id());
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
