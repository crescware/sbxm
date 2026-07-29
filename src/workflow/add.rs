//! `sbxm add`。
//!
//! 新しいGitHub repositoryを管理対象へ登録し、構築が中断した案件には同じcommandで
//! 続きから進む。全工程を`inspect -> decide -> mutate -> verify -> record`で実行し、
//! 成功済みの成果物をrollback目的で削除しない。

use std::fs;
use std::path::PathBuf;

use crate::command::HostEnvironment;
use crate::config::GlobalConfig;
use crate::error::{Diagnostic, Error, ErrorId, Msg, Result, fail};
use crate::git;
use crate::hash::sha256_hex;
use crate::metadata::{
    self, CreationMode, MAX_WORKTREES, MIN_WORKTREES, ProjectMetadata, Provisioning,
};
use crate::msg;
use crate::paths::{
    self, ExclusiveLock, PRIVATE_DIR_MODE, PRIVATE_FILE_MODE, PathScope, ProjectPaths,
};
use crate::project::{ProjectId, SandboxName};

use super::{host_clone, rebuild};

/// 案件のDockerfileを新規作成するときの初期template。
///
/// 作成後は利用者が管理するfileであり、sbxmは内容を変更しない。変更の適用は
/// `rebuild`が担当する。
const BUNDLED_DOCKERFILE: &str = include_str!("../../assets/Dockerfile");

/// `add`の入力。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddRequest {
    pub project: ProjectId,
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

        match &request.detach {
            Some(branch) => {
                git::validate_branch_name(branch)?;
                Ok(TargetConfiguration {
                    mode: CreationMode::Detached,
                    start_ref: Some(branch.clone()),
                    requested_worktrees,
                })
            }
            None => {
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
/// 1. 入力を検証し、既存案件との衝突検査を完了する
/// 2. owner directory、project root、`.sbxm`、`.cache`を作る
/// 3. project lockを取得する
/// 4. Dockerfileがなければbundled templateから作り、あれば内容を変えず採用する
/// 5. 目標構成を含むmetadataをatomic writeする
pub fn register(config: &GlobalConfig, request: &AddRequest) -> Result<Registration> {
    let target = TargetConfiguration::from_request(request)?;
    let canonical = request.project.canonical();
    let sandbox = SandboxName::derive(&canonical);

    // 破損した案件が1件でもあれば、一覧を部分的に信用せずここで停止する。
    let known = metadata::discover(&config.base_path)?;
    if let Some(other) = known.iter().find(|project| {
        project.metadata.canonical_id != canonical && project.metadata.sandbox_name() == sandbox
    }) {
        return fail(
            ErrorId::SandboxNameCollision,
            msg!(
                "error-sandbox-name-collision",
                sandbox = sandbox,
                projects = format!("{}, {}", canonical, other.metadata.canonical_id)
            ),
        );
    }
    if let Some(project) = known
        .iter()
        .find(|project| project.metadata.canonical_id == canonical)
    {
        // mutationの前に、保存済み目標構成との一致を判定する。
        check_continuable(&project.metadata, request)?;
    }

    let paths = ProjectPaths::derive(&config.base_path, &canonical);
    paths::ensure_directory(paths.owner_dir())?;
    paths::ensure_directory(paths.root())?;
    paths::ensure_private_dir(&paths.sbxm_dir(), PRIVATE_DIR_MODE, PathScope::ProjectPath)?;
    paths::ensure_private_dir(&paths.cache_dir(), PRIVATE_DIR_MODE, PathScope::ProjectPath)?;

    let lock = paths.acquire_lock()?;

    // lock取得後にmetadataを取り直し、preconditionを判定し直す。
    let stored = metadata::load(&paths)?;
    if let Some(stored) = &stored {
        check_continuable(stored, request)?;
    }

    let dockerfile_sha256 = adopt_dockerfile(&paths)?;

    let metadata = match stored {
        Some(stored) => stored,
        None => {
            let metadata = ProjectMetadata {
                owner: request.project.owner().to_string(),
                repository: request.project.repository().to_string(),
                canonical_id: canonical,
                provisioning: Provisioning {
                    mode: target.mode,
                    start_ref: target.start_ref,
                    requested_worktrees: target.requested_worktrees,
                    dockerfile_sha256: dockerfile_sha256.clone(),
                },
                rebuild: None,
            };
            metadata::create(&paths, &metadata)?;
            metadata
        }
    };

    Ok(Registration {
        paths,
        sandbox,
        metadata,
        _lock: lock,
    })
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
    pub warnings: Vec<Msg>,
}

/// 案件を管理下へ置き、host cloneを用意する。
///
/// Sandboxは作らない。Sandbox名はcanonical project IDから決まるため、この時点で
/// 確定して呼び出し側へ返せる。GitHub tokenの登録先がその名前になる。
pub fn run(
    config: &GlobalConfig,
    request: &AddRequest,
    host: &dyn HostEnvironment,
) -> Result<AddOutput> {
    let paths = ProjectPaths::derive(&config.base_path, &request.project.canonical());
    let already_registered = metadata::load(&paths)?.is_some();

    let registration = register(config, request)?;
    // host cloneは利用者のSSH鍵でhost上から取る。Sandboxのsecretは要らない。
    let clone = host_clone::ensure(host, &registration.paths, &request.project)?;

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

/// 現在のDockerfileのSHA-256。
///
/// `add`が採用したfileを、以後の工程が世代の正本として読む。
pub fn current_dockerfile_hash(paths: &ProjectPaths) -> Result<String> {
    let path = paths.dockerfile();
    if !paths::regular_file_exists(&path, PathScope::ProjectPath)? {
        return Err(Error::new(
            ErrorId::ProjectPathUnreadable,
            msg!(
                "error-project-path-unreadable",
                path = paths::display(&path),
                detail = "the Dockerfile of this project is absent"
            ),
        ));
    }
    let contents = std::fs::read(&path)
        .map_err(|error| PathScope::ProjectPath.unreadable_error(&path, &error.to_string()))?;
    Ok(sha256_hex(&contents))
}

/// 保存済みmetadataを持つ案件で、この`add`が構築を続けてよいかを判定する。
///
/// 省略されたoptionは保存値を使う。指定されたoptionは保存値との完全一致を要求する。
fn check_continuable(stored: &ProjectMetadata, request: &AddRequest) -> Result<()> {
    let display_id = stored.display_id();

    // 世代の切替中であり、初回構築の継続とは別の工程が必要になる。
    rebuild::require_no_rebuild(stored)?;

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
            .remediation(msg!(
                "remediation-target-configuration-mismatch",
                command = format!("sbxm add {display_id}")
            )),
        ))
    };

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
#[path = "add_test.rs"]
mod add_test;
