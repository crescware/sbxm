//! 管理案件とSandboxの突き合わせ。
//!
//! runtime状態を複製せず、command実行のたびに1回のSandbox一覧取得から組み立てる。
//! 名前は完全一致で突き合わせ、substringや表示textの検索は使わない。

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::command::{CommandSpec, EnvPolicy, HostEnvironment, TimeoutClass};
use crate::compatibility::{SandboxEntry, SandboxState};
use crate::config::ConfigLocation;
use crate::error::{Diagnostic, Error, ErrorId, Result};
use crate::metadata::{self, ProjectMetadata};
use crate::msg;
use crate::paths::ProjectPaths;
use crate::project::SandboxName;
use crate::registry::{self, RegistryEntry};

use super::{daemon, sandbox};
use crate::ui::ProgressSink;
use crate::ui::Remediation;

/// 状態が変わるのを待つ間隔と上限。
///
/// 起動と停止の完了は、commandの戻り値ではなく一覧のstateを読み直して判定する。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Poll {
    pub interval: Duration,
    pub limit: Duration,
}

impl Default for Poll {
    fn default() -> Poll {
        Poll {
            interval: Duration::from_secs(2),
            limit: Duration::from_secs(60),
        }
    }
}

/// 利用者へ見せるSandboxの状態。
///
/// `registered`は内部の管理状態名であり、対応Sandboxがまだない同じ状態を
/// 利用者向けには`not-created`と表示する。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectState {
    NotCreated,
    Running,
    Stopped,
}

impl ProjectState {
    /// 翻訳しない安定した表記。
    pub fn as_str(self) -> &'static str {
        match self {
            ProjectState::NotCreated => "not-created",
            ProjectState::Running => "running",
            ProjectState::Stopped => "stopped",
        }
    }

    /// 凡例に使うFTL message ID。host serviceではなくSandboxの状態を説明する。
    pub fn legend_id(self) -> &'static str {
        match self {
            ProjectState::NotCreated => "legend-not-created",
            ProjectState::Running => "legend-sandbox-running",
            ProjectState::Stopped => "legend-sandbox-stopped",
        }
    }
}

impl std::fmt::Display for ProjectState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// registry entryを起点に観測した、1案件の現在の状態。
///
/// 状態名を永続化せず、entry、project root、metadata、Sandbox一覧という事実から
/// そのつど算出する。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Observed {
    /// project rootが無い。作成前に中断したか、あとから移動・削除された。
    Missing,
    /// project rootはあるが、metadataがまだ無い。
    Incomplete,
    /// metadataがregistry entryと一致しない、または読めない。
    Inconsistent,
    /// 一致するmetadataがあり、Sandboxの状態まで観測できた。
    Registered(ProjectState),
}

impl Observed {
    /// 翻訳しない安定した表記。
    pub fn as_str(&self) -> &'static str {
        match self {
            Observed::Missing => "missing",
            Observed::Incomplete => "incomplete",
            Observed::Inconsistent => "inconsistent",
            Observed::Registered(state) => state.as_str(),
        }
    }

    /// 凡例に使うFTL message ID。
    pub fn legend_id(&self) -> &'static str {
        match self {
            Observed::Missing => "legend-missing",
            Observed::Incomplete => "legend-incomplete",
            Observed::Inconsistent => "legend-inconsistent",
            Observed::Registered(state) => state.legend_id(),
        }
    }

    /// 登録が完了し、成果物がentryと一致しているか。
    pub fn is_settled(&self) -> bool {
        matches!(self, Observed::Registered(_))
    }
}

/// 1件の管理案件と、その現在の状態。
#[derive(Debug, Clone)]
pub struct ManagedProject {
    /// 表示に使う`<owner>/<repository>`。registry entryから決まる。
    pub display_id: String,
    pub project_root: PathBuf,
    pub sandbox: String,
    pub observed: Observed,
}

/// 管理案件と、管理外のSandbox。
#[derive(Debug, Clone)]
pub struct Inventory {
    /// canonical ID byte昇順。
    pub projects: Vec<ManagedProject>,
    /// Sandbox name byte昇順。
    pub unmanaged: Vec<SandboxEntry>,
}

impl Inventory {
    /// 全案件が登録済みで、entryと成果物が一致しているか。
    pub fn is_settled(&self) -> bool {
        self.projects
            .iter()
            .all(|project| project.observed.is_settled())
    }
}

/// 1案件の現在の状態を、取得済みの一覧から決める。
///
/// 一覧全体が成立している必要はない。対象と無関係な案件の破損によって、この案件の
/// 状態が読めなくなることを避ける。対応が矛盾する場合は、正常な状態として返さない。
pub fn state_of(
    entries: &[SandboxEntry],
    metadata: &ProjectMetadata,
    workspace_root: &Path,
) -> Result<ProjectState> {
    let name = metadata.sandbox_name();
    let Some(entry) = single(entries, name.as_str())? else {
        return Ok(ProjectState::NotCreated);
    };
    sandbox::verify_identity(entry, &name, workspace_root)?;
    Ok(match entry.state {
        SandboxState::Running => ProjectState::Running,
        SandboxState::Stopped => ProjectState::Stopped,
    })
}

/// Sandboxをまだ持たない案件を、構築commandとともに拒否する。
pub fn not_created(metadata: &ProjectMetadata, sandbox: &str) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::SandboxNotCreated,
            msg!(
                "error-sandbox-not-created",
                project = metadata.display_id(),
                sandbox = sandbox
            ),
        )
        // 案件は既に登録済みである。`add`はimageにもsandboxにも触れないため、
        // 構築するcommandを案内する。
        .remediation(
            Remediation::text(msg!("remediation-sandbox-not-created"))
                .try_run(format!("sbxm prepare {}", metadata.display_id())),
        ),
    )
}

/// 名前が一致するentryを1件だけ取り出す。
///
/// 同名が複数ある一覧からは、どれがこの案件のSandboxかを決められない。先頭を選んで
/// 続けると、別のSandboxのsessionやstateを読んだまま削除へ進み得る。
pub fn single<'a>(entries: &'a [SandboxEntry], name: &str) -> Result<Option<&'a SandboxEntry>> {
    let matched: Vec<&SandboxEntry> = entries.iter().filter(|entry| entry.name == name).collect();
    match matched.as_slice() {
        [] => Ok(None),
        [entry] => Ok(Some(entry)),
        _ => Err(duplicated(&[name])),
    }
}

/// 非対話でSandboxを起動する。
pub fn start(
    host: &dyn HostEnvironment,
    sandbox: &str,
    progress: &mut dyn ProgressSink,
) -> Result<()> {
    progress.step(msg!("progress-starting-sandbox"));
    let spec = CommandSpec::passthrough("sbx", &["exec", sandbox, "--", "/bin/true"])
        .env(EnvPolicy::InheritWithoutSshAgent)
        .timeout(TimeoutClass::SandboxLifecycle);
    host.run(&spec)?.require_success()?;
    Ok(())
}

/// runningになるまで待つ。状態は毎回structured outputから読む。
pub fn wait_until_running(
    host: &dyn HostEnvironment,
    metadata: &ProjectMetadata,
    workspace_root: &Path,
    poll: Poll,
) -> Result<()> {
    let name = metadata.sandbox_name();
    let deadline = Instant::now() + poll.limit;
    loop {
        let entries = daemon::list(host)?;
        let observed = state_of(&entries, metadata, workspace_root)?;
        if observed == ProjectState::Running {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(Error::single(
                Diagnostic::new(
                    ErrorId::SandboxNotRunning,
                    msg!(
                        "error-sandbox-not-running",
                        sandbox = name,
                        observed = observed
                    ),
                )
                .remediation(
                    Remediation::text(msg!("remediation-diagnose-project"))
                        .try_run(format!("sbxm status {}", metadata.display_id())),
                ),
            ));
        }
        std::thread::sleep(poll.interval);
    }
}

/// Sandboxを削除し、一覧から消えるまで待つ。
///
/// commandの戻り値だけを不在の根拠にしない。`force`はデータ保護検査を省略した
/// 削除であり、runtimeへ渡す引数だけが変わる。
pub fn remove(
    host: &dyn HostEnvironment,
    name: &SandboxName,
    poll: Poll,
    progress: &mut dyn ProgressSink,
) -> Result<()> {
    // `--force`が省くのは`sbx`の確認promptだけである。削除してよいかはsbxmが先に
    // 判定しており、`destroy`は自前の確認も済ませている。非対話で走る実行では
    // promptに答える手段がなく、対話実行でも二度訊くことになる。
    progress.step(msg!("progress-removing-sandbox"));
    let args = ["rm", "--force", name.as_str()];
    let spec = CommandSpec::passthrough("sbx", &args)
        .env(EnvPolicy::InheritWithoutSshAgent)
        .timeout(TimeoutClass::SandboxLifecycle);
    host.run(&spec)?.require_success()?;

    let deadline = Instant::now() + poll.limit;
    loop {
        if single(&daemon::list(host)?, name.as_str())?.is_none() {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(still_present(name));
        }
        std::thread::sleep(poll.interval);
    }
}

/// 削除したはずのSandboxが一覧に残っている。
pub fn still_present(name: &SandboxName) -> Error {
    Error::new(
        ErrorId::SandboxStillPresent,
        msg!("error-sandbox-still-present", sandbox = name),
    )
}

/// 現在のinventoryを1回の一覧取得から組み立てる。
///
/// registryが読めない、Sandbox名が重複、対応が矛盾する場合は、部分的に正しそうな
/// 結果を返さずerrorとする。個々の案件の観測結果は、entryを黙って落とさずそのまま返す。
pub fn take(
    location: &ConfigLocation,
    host: &dyn HostEnvironment,
    workspace_root: &Path,
) -> Result<Inventory> {
    let registry = registry::load(location)?;
    let entries = daemon::list(host)?;
    require_unique_names(&entries)?;

    let mut projects = Vec::with_capacity(registry.entries().len());
    let mut claimed: BTreeSet<String> = BTreeSet::new();
    for entry in registry.entries() {
        let name = entry.sandbox_name();
        let paths = ProjectPaths::at(entry.project_root(), entry.canonical_id());
        let observed = observe(&paths, entry, &entries, workspace_root)?;
        if matches!(
            observed,
            Observed::Registered(ProjectState::Running | ProjectState::Stopped)
        ) {
            claimed.insert(name.as_str().to_string());
        }
        projects.push(ManagedProject {
            display_id: entry.repository().display_id(),
            project_root: entry.project_root().to_path_buf(),
            sandbox: name.as_str().to_string(),
            observed,
        });
    }

    let mut unmanaged: Vec<SandboxEntry> = entries
        .into_iter()
        .filter(|entry| !claimed.contains(&entry.name))
        .collect();
    unmanaged.sort_by(|left, right| left.name.as_bytes().cmp(right.name.as_bytes()));

    Ok(Inventory {
        projects,
        unmanaged,
    })
}

/// 1件のregistry entryが指す成果物を観測する。
///
/// entryを黙って削除せず、観測できた事実をそのまま返す。metadataが読めないことと
/// 一致しないことは、どちらもentryを信用できない状態として`inconsistent`とする。
fn observe(
    paths: &ProjectPaths,
    entry: &RegistryEntry,
    sandboxes: &[SandboxEntry],
    workspace_root: &Path,
) -> Result<Observed> {
    if !paths.root().is_dir() {
        return Ok(Observed::Missing);
    }
    let metadata = match metadata::load(paths) {
        Ok(Some(metadata)) => metadata,
        Ok(None) => return Ok(Observed::Incomplete),
        Err(_) => return Ok(Observed::Inconsistent),
    };
    if !metadata.repository.same_target(entry.repository()) {
        return Ok(Observed::Inconsistent);
    }
    Ok(Observed::Registered(state_of(
        sandboxes,
        &metadata,
        workspace_root,
    )?))
}

/// 同名のSandboxが複数ある一覧からは、対応を決められない。
fn require_unique_names(entries: &[SandboxEntry]) -> Result<()> {
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    let mut duplicates: BTreeSet<&str> = BTreeSet::new();
    for entry in entries {
        if !seen.insert(entry.name.as_str()) {
            duplicates.insert(entry.name.as_str());
        }
    }
    if duplicates.is_empty() {
        return Ok(());
    }
    Err(duplicated(&duplicates.into_iter().collect::<Vec<&str>>()))
}

/// 一覧に同じ名前のSandboxが複数あることを、そのまま伝える。
fn duplicated(names: &[&str]) -> Error {
    Error::new(
        ErrorId::SandboxNameCollision,
        msg!("error-sandbox-name-duplicated", sandbox = names.join(", ")),
    )
}

#[cfg(test)]
#[path = "inventory_test.rs"]
mod inventory_test;
