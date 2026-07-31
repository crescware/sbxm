//! `sbxm add`。
//!
//! `新しいGitHub` repositoryを管理対象へ登録し、構築が中断した案件には同じcommandで
//! 続きから進む。全工程を`inspect -> decide -> mutate -> verify -> record`で実行し、
//! 成功済みの成果物をrollback目的で削除しない。

use std::fs;
use std::path::PathBuf;

use crate::command::HostEnvironment;
use crate::config::ConfigLocation;
use crate::error::{Diagnostic, Error, ErrorId, Result, fail};
use crate::git;
use crate::hash::sha256_hex;
use crate::metadata::{
    self, CreationMode, GitIdentity, MAX_WORKTREES, MIN_WORKTREES, ProjectMetadata, Provisioning,
};
use crate::msg;
use crate::paths::{
    self, ExclusiveLock, PRIVATE_DIR_MODE, PRIVATE_FILE_MODE, PathScope, ProjectParent,
    ProjectPaths,
};
use crate::project::{CanonicalProjectId, SandboxName};
use crate::registry::{self, Registry, RegistryEntry, RegistryGuard};
use crate::repository::RepositoryIdentity;

use crate::support::generation;

use super::host_clone;
use crate::ui::{ProgressSink, Remediation, Warning};

/// 案件のDockerfileを新規作成するときの初期template。
///
/// 作成後は利用者が管理するfileであり、sbxmは内容を変更しない。変更の適用は
/// `rebuild`が担当する。
const BUNDLED_DOCKERFILE: &str = include_str!("../../../assets/Dockerfile");

/// `add`の入力。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddRequest {
    /// clone URLから解釈した登録対象。
    pub repository: RepositoryIdentity,
    pub worktrees: Option<u32>,
    pub detach: Option<String>,
}

/// optionから決まる目標構成。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetConfiguration {
    pub mode: CreationMode,
    pub start_ref: Option<String>,
    pub requested_worktrees: u32,
}

impl TargetConfiguration {
    /// | 指定 | mode | start_ref | managed数 |
    /// |---|---|---|---:|
    /// | 指定なし | attached | remote default branch | 1 |
    /// | `--detach BRANCH` | detached | BRANCH | 1 |
    /// | `--worktrees N --detach BRANCH` | detached | BRANCH | N |
    pub fn from_request(request: &AddRequest) -> Result<TargetConfiguration> {
        let requested_worktrees = request.worktrees.unwrap_or(MIN_WORKTREES);
        if !(MIN_WORKTREES..=MAX_WORKTREES).contains(&requested_worktrees) {
            return fail(
                ErrorId::WorktreesOutOfRange,
                msg!(
                    "error-worktrees-out-of-range",
                    value = requested_worktrees,
                    minimum = MIN_WORKTREES,
                    maximum = MAX_WORKTREES
                ),
            );
        }

        if let Some(branch) = &request.detach {
            git::validate_branch_name(branch)?;
            Ok(TargetConfiguration {
                mode: CreationMode::Detached,
                start_ref: Some(branch.clone()),
                requested_worktrees,
            })
        } else {
            if requested_worktrees > 1 {
                return fail(
                    ErrorId::WorktreesRequireDetach,
                    msg!("error-worktrees-require-detach"),
                );
            }
            Ok(TargetConfiguration {
                // attached modeのstart refはremote default branchを解決してから確定する。
                mode: CreationMode::Attached,
                start_ref: None,
                requested_worktrees,
            })
        }
    }
}

/// 登録を終えた案件。
///
/// project lockを保持しているため、この値が生きているあいだ同じ案件へのmutationは
/// 直列化される。
#[derive(Debug)]
pub struct Registration {
    pub paths: ProjectPaths,
    pub sandbox: SandboxName,
    pub metadata: ProjectMetadata,
    _lock: ExclusiveLock,
}

/// 案件を登録し、以後の外部mutationへ進める状態にする。
///
/// 1. global registry lockを取得し、document全体を検証する
/// 2. registry全体と新規要求の衝突を検査する
/// 3. 登録意図を持つentryをregistryへatomic recordする
/// 4. registry lockを保持したままproject rootを作り、project lockを取得する
/// 5. Dockerfileとproject metadataをatomic createする
/// 6. entryとproject metadataを再検証する
/// 7. registry lockだけを解放する
///
/// 長時間かかるhost cloneは、registry lockを解放したあとで`run`が行う。
pub fn register(
    location: &ConfigLocation,
    parent: &ProjectParent,
    request: &AddRequest,
    git_identity: &GitIdentity,
) -> Result<Registration> {
    let target = TargetConfiguration::from_request(request)?;
    let canonical = request.repository.canonical_id().clone();
    let sandbox = SandboxName::derive(&canonical);

    // registryが不正なら、一部entryだけを信用せずここで停止する。
    let mut guard = RegistryGuard::acquire(location)?;

    let paths = if let Some(entry) = guard.registry().find(&canonical) {
        // 登録済みなら、実行時の配置規則から新しい候補pathを作らない。
        require_same_registration(entry, &request.repository)?;
        let paths = ProjectPaths::at(entry.project_root(), &canonical);
        // 保存されたabsolute pathでも、そこにあるものを観測してから使う。
        // rootがまだ無い中断点からの再開だけが、作成工程から続けられる。
        match observe(paths.root())? {
            Presence::Present => {
                paths::require_owned_directory(paths.root(), PathScope::ProjectPath)?;
            }
            Presence::Absent => {}
        }
        paths
    } else {
        // cwdを使うのは新規canonical project IDの登録時だけである。
        let candidate = ProjectPaths::derive(parent, &canonical);
        check_new_registration(guard.registry(), &candidate, &canonical, &sandbox)?;
        guard.insert(RegistryEntry::new(
            candidate.root(),
            request.repository.clone(),
        )?)?;
        candidate
    };

    paths::ensure_directory(paths.root())?;
    paths::ensure_private_dir(&paths.sbxm_dir(), PRIVATE_DIR_MODE, PathScope::ProjectPath)?;
    paths::ensure_private_dir(&paths.cache_dir(), PRIVATE_DIR_MODE, PathScope::ProjectPath)?;

    let lock = paths.acquire_lock()?;

    // lock取得後にmetadataを取り直し、preconditionを判定し直す。
    let stored = metadata::load(&paths)?;
    if let Some(stored) = &stored {
        if *stored.canonical_id() != canonical {
            return Err(path_collision(
                paths.root(),
                stored.canonical_id(),
                &canonical,
            ));
        }
        check_continuable(stored, request)?;
    }

    let dockerfile_sha256 = adopt_dockerfile(&paths)?;

    let metadata = if let Some(stored) = stored {
        stored
    } else {
        let metadata = ProjectMetadata {
            repository: request.repository.clone(),
            provisioning: Provisioning {
                mode: target.mode,
                start_ref: target.start_ref,
                requested_worktrees: target.requested_worktrees,
                dockerfile_sha256: dockerfile_sha256.clone(),
            },
            git_identity: git_identity.clone(),
            rebuild: None,
        };
        metadata::create(&paths, &metadata)?;
        metadata
    };

    // entryとmetadataが同じ案件を指していることを、registry lockを手放す前に確かめる。
    let entry = guard
        .registry()
        .find(&canonical)
        .ok_or_else(|| missing_entry(&canonical))?;
    if entry.project_root() != paths.root() {
        return Err(path_collision(
            paths.root(),
            entry.canonical_id(),
            &canonical,
        ));
    }
    require_same_registration(entry, &metadata.repository)?;
    drop(guard);

    Ok(Registration {
        paths,
        sandbox,
        metadata,
        _lock: lock,
    })
}

/// 新規登録として、この候補pathを使ってよいかを判定する。
///
/// registry entryのない既存成果物のownershipは`add`では確定できない。名前が一致する
/// だけのdirectoryをadoptせず、path collisionとして拒否する。
fn check_new_registration(
    registry: &Registry,
    candidate: &ProjectPaths,
    canonical: &CanonicalProjectId,
    sandbox: &SandboxName,
) -> Result<()> {
    if let Some(other) = registry
        .entries()
        .iter()
        .find(|entry| entry.project_root() == candidate.root())
    {
        return Err(path_collision(
            candidate.root(),
            other.canonical_id(),
            canonical,
        ));
    }
    if let Some(other) = registry
        .entries()
        .iter()
        .find(|entry| entry.sandbox_name() == *sandbox)
    {
        return fail(
            ErrorId::SandboxNameCollision,
            msg!(
                "error-sandbox-name-collision",
                sandbox = sandbox,
                projects = format!("{}, {}", canonical, other.canonical_id())
            ),
        );
    }
    // 観測できないことを、空いていることと同一視しない。
    match observe(candidate.root())? {
        Presence::Absent => Ok(()),
        Presence::Present => Err(Error::single(
            Diagnostic::new(
                ErrorId::ProjectPathCollision,
                msg!(
                    "error-project-path-occupied",
                    path = paths::display(candidate.root()),
                    requested = canonical
                ),
            )
            .remediation(msg!(
                "remediation-project-path-occupied",
                path = paths::display(candidate.root())
            )),
        )),
    }
}

/// pathに何かがあるか。
enum Presence {
    Present,
    Absent,
}

/// symlinkを追跡せずにpathの有無を観測する。
///
/// 不在と、観測できないことを区別する。読めないpathを空いているものとして扱わない。
fn observe(path: &std::path::Path) -> Result<Presence> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => Ok(Presence::Present),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Presence::Absent),
        Err(error) => Err(PathScope::ProjectPath.unreadable_error(path, &error.to_string())),
    }
}

/// registry entryが、この実行の登録対象と同じ構成を指しているか。
fn require_same_registration(entry: &RegistryEntry, requested: &RepositoryIdentity) -> Result<()> {
    if entry.repository().same_target(requested) {
        return Ok(());
    }
    Err(Error::single(
        Diagnostic::new(
            ErrorId::TargetConfigurationMismatch,
            msg!(
                "error-target-configuration-mismatch",
                project = entry.repository().display_id(),
                requested = requested.clone_url(),
                stored = entry.repository().clone_url()
            ),
        )
        .remediation(
            Remediation::text(msg!("remediation-target-configuration-mismatch"))
                .try_run(format!("sbxm add {}", entry.repository().clone_url())),
        ),
    ))
}

/// 記録したはずのentryが、lockを保持したまま読み直したregistryにない。
fn missing_entry(canonical: &CanonicalProjectId) -> Error {
    Error::new(
        ErrorId::RegistryEntryMismatch,
        msg!(
            "error-registry-entry-mismatch",
            canonical_id = canonical,
            stored = "-",
            requested = canonical
        ),
    )
}

/// `add`の結果。
///
/// 案件を管理下へ置き、host cloneを用意したところまでを示す。Sandboxはまだ存在せず、
/// 構築は`prepare`が行う。
#[derive(Debug, Clone)]
pub struct AddOutput {
    pub project: String,
    /// 構築で使うSandbox名。canonical project IDから決まり、この時点では未作成。
    pub sandbox: String,
    pub mode: CreationMode,
    /// 起点branch。attached modeは構築時にremoteから解決するため`None`のことがある。
    pub start_ref: Option<String>,
    pub requested_worktrees: u32,
    pub host_clone: PathBuf,
    /// 既に登録済みで、この実行が目標構成を変えなかったか。
    pub already_registered: bool,
    pub warnings: Vec<Warning>,
}

/// 案件を管理下へ置き、host cloneを用意する。
///
/// Sandboxは作らない。Sandbox名はcanonical project IDから決まるため、この時点で
/// 確定して呼び出し側へ返せる。GitHub tokenの登録先がその名前になる。
pub fn run(
    location: &ConfigLocation,
    parent: &ProjectParent,
    request: &AddRequest,
    git_identity: &GitIdentity,
    host: &dyn HostEnvironment,
    progress: &mut dyn ProgressSink,
) -> Result<AddOutput> {
    let already_registered = was_already_registered(location, &request.repository)?;

    // Sandbox内で使うidentityは、案件を作る前に呼び出し側が決めている。
    let registration = register(location, parent, request, git_identity)?;
    // host cloneは、validation済みの入力と同じtransportとclone URLで取る。
    let clone = host_clone::ensure(
        host,
        &registration.paths,
        &registration.metadata.repository,
        progress,
    )?;

    let provisioning = &registration.metadata.provisioning;
    Ok(AddOutput {
        project: registration.metadata.display_id(),
        sandbox: registration.sandbox.as_str().to_string(),
        mode: provisioning.mode,
        start_ref: provisioning.start_ref.clone(),
        requested_worktrees: provisioning.requested_worktrees,
        host_clone: clone.path,
        already_registered,
        warnings: Vec::new(),
    })
}

/// この実行の前から、metadataまで揃った登録済み案件だったか。
///
/// registry entryのabsolute pathを、読む前に観測する。保存されたrootであっても、
/// そこにdirectoryがあり、現在の利用者が所有していることを確かめてからmetadataを読む。
/// rootがまだ無い中断点は登録途中であり、観測できないrootは不在と同一視しない。
fn was_already_registered(
    location: &ConfigLocation,
    repository: &RepositoryIdentity,
) -> Result<bool> {
    let registry = registry::load(location)?;
    let Some(entry) = registry.find(repository.canonical_id()) else {
        return Ok(false);
    };
    let paths = ProjectPaths::at(entry.project_root(), entry.canonical_id());
    match observe(paths.root())? {
        Presence::Absent => Ok(false),
        Presence::Present => {
            paths::require_owned_directory(paths.root(), PathScope::ProjectPath)?;
            Ok(metadata::load(&paths)?.is_some())
        }
    }
}

/// 保存済みmetadataを持つ案件で、この`add`が構築を続けてよいかを判定する。
///
/// 省略されたoptionは保存値を使う。指定されたoptionは保存値との完全一致を要求する。
/// 登録済みのrepository identityは、transportまで含めた完全一致を要求する。
fn check_continuable(stored: &ProjectMetadata, request: &AddRequest) -> Result<()> {
    let display_id = stored.display_id();
    let registered_url = stored.repository.clone_url().to_string();

    // 世代の切替中であり、初回構築の継続とは別の工程が必要になる。
    generation::require_no_rebuild(stored)?;

    let provisioning = &stored.provisioning;
    let mismatch = |requested: String, stored: String| {
        Err(Error::single(
            Diagnostic::new(
                ErrorId::TargetConfigurationMismatch,
                msg!(
                    "error-target-configuration-mismatch",
                    project = display_id,
                    requested = requested,
                    stored = stored
                ),
            )
            .remediation(
                Remediation::text(msg!("remediation-target-configuration-mismatch"))
                    // 保存済みの綴りをそのまま示す。再実行で登録内容を書き換えさせない。
                    .try_run(format!("sbxm add {registered_url}")),
            ),
        ))
    };

    // 同じcanonical project IDでも、SSHとHTTPSを同一構成とみなさない。
    if !stored.repository.same_target(&request.repository) {
        return mismatch(
            request.repository.clone_url().to_string(),
            stored.repository.clone_url().to_string(),
        );
    }

    if let Some(branch) = &request.detach {
        let stored_branch = provisioning.start_ref.clone().unwrap_or_default();
        if provisioning.mode != CreationMode::Detached || stored_branch != *branch {
            return mismatch(
                format!("{} {branch}", CreationMode::Detached),
                format!("{} {stored_branch}", provisioning.mode),
            );
        }
    }
    if let Some(worktrees) = request.worktrees
        && provisioning.requested_worktrees != worktrees
    {
        return mismatch(
            format!("{worktrees} worktrees"),
            format!("{} worktrees", provisioning.requested_worktrees),
        );
    }
    Ok(())
}

/// 同じproject rootを別の案件が既に使っている。
///
/// owner名などを自動的に加えて衝突を回避しない。別の親directoryで登録するよう案内する。
fn path_collision(
    root: &std::path::Path,
    occupant: &CanonicalProjectId,
    requested: &CanonicalProjectId,
) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::ProjectPathCollision,
            msg!(
                "error-project-path-collision",
                path = paths::display(root),
                observed = occupant,
                requested = requested
            ),
        )
        .remediation(msg!("remediation-project-path-collision")),
    )
}

/// Dockerfileを採用し、そのSHA-256を返す。
///
/// 既存fileは利用者が管理・編集するものとして内容を変更せず採用する。
fn adopt_dockerfile(paths: &ProjectPaths) -> Result<String> {
    let path = paths.dockerfile();
    if paths::regular_file_exists(&path, PathScope::ProjectPath)? {
        let contents = fs::read(&path)
            .map_err(|error| PathScope::ProjectPath.unreadable_error(&path, &error.to_string()))?;
        return Ok(sha256_hex(&contents));
    }
    paths::atomic_create(&path, BUNDLED_DOCKERFILE, PRIVATE_FILE_MODE)?;
    Ok(sha256_hex(BUNDLED_DOCKERFILE.as_bytes()))
}

#[cfg(test)]
#[path = "run_test.rs"]
mod run_test;
