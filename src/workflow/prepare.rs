//! `sbxm prepare`。
//!
//! 登録済み案件のSandboxを作り、作業できる状態にする。案件の登録とhost cloneは
//! `add`が終えており、ここから先は目標構成をmetadataから読む。
//!
//! 中断した案件へ同じcommandを再実行すると、成功済みの工程をinspectしてskipし、
//! 最初の未完了工程から続ける。

use std::path::Path;

use crate::command::HostEnvironment;
use crate::config::GlobalConfig;
use crate::error::{Diagnostic, Error, ErrorId, Msg, Result};
use crate::metadata::{self, CreationMode, ProjectMetadata};
use crate::msg;
use crate::paths::{self, LOCK_TIMEOUT, PRIVATE_FILE_MODE, PathScope, ProjectPaths};
use crate::project::{ProjectId, SandboxLayout, SandboxName};

use super::files::PlacedFile;
use super::tools::Note;
use super::{daemon, files, identity, image, repository, sandbox, secret, template, tools};

/// 出力のworktree 1行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeRow {
    pub path: String,
    pub created_from: String,
    /// 観測できたHEAD。停止中のSandboxでは読めないため`None`になる。
    pub head: Option<String>,
    pub mode: CreationMode,
}

/// `prepare`の結果。
#[derive(Debug, Clone)]
pub struct PrepareOutput {
    pub project: String,
    pub sandbox: String,
    pub mode: CreationMode,
    pub start_ref: String,
    pub sandbox_state: crate::compatibility::SandboxState,
    pub worktrees: Vec<WorktreeRow>,
    pub files: Vec<PlacedFile>,
    /// Sandboxに入っているtoolが返した案内。sbxmが代わりに実行しないことを示す。
    pub notes: Vec<Note>,
    /// 既に構築済みで、この実行が何も変更しなかったか。
    pub already_built: bool,
    pub warnings: Vec<Msg>,
}

/// 登録済み案件のSandboxを構築する。
pub fn run(
    config: &GlobalConfig,
    project: &ProjectId,
    host: &dyn HostEnvironment,
    workspace_root: &Path,
) -> Result<PrepareOutput> {
    let canonical = project.canonical();
    let paths = ProjectPaths::derive(&config.base_path, &canonical);
    let name = SandboxName::derive(&canonical);

    // 対象が登録されていない案件にlock fileを作らない。
    if metadata::load(&paths)?.is_none() {
        return Err(not_registered(project));
    }
    let _lock = paths::acquire_exclusive_lock(
        &paths.lock_file(),
        LOCK_TIMEOUT,
        PRIVATE_FILE_MODE,
        PathScope::ProjectPath,
    )?;

    // lockを取る前に読んだmetadataは古くなり得る。判定はlock後の内容だけで行う。
    let mut project_metadata = metadata::load(&paths)?.ok_or_else(|| not_registered(project))?;
    require_no_rebuild(&project_metadata)?;

    let layout = SandboxLayout::new(&canonical);
    let mut warnings = Vec::new();

    if let Some(output) = already_built(
        host,
        &paths,
        &name,
        &project_metadata,
        &layout,
        workspace_root,
    )? {
        return Ok(output);
    }

    // custom secretはSandboxの作成時に結び付く。あとから登録しても既存のSandboxには
    // 届かないため、作成より前に、そしてimageを組む前に確認する。
    secret::require_github(host, name.as_str())?;

    let current = super::add::current_dockerfile_hash(&paths)?;
    let generation = adopt_generation(
        host,
        &paths,
        &mut project_metadata,
        &name,
        &current,
        &mut warnings,
    )?;

    let built = image::ensure(
        host,
        &name,
        &project_metadata.canonical_id,
        &paths.dockerfile(),
        &generation,
    )?;
    warnings.extend(built.warnings.clone());
    let archive = image::ensure_archive(host, &paths, &built, &generation)?;
    let loaded = template::ensure(host, &archive, &built)?;

    let ready = sandbox::ensure(host, &name, &loaded, workspace_root)?;
    // hostのSSH Agentが届かないことを、daemonの起動条件から推定せず中から確かめる。
    sandbox::require_credentials_isolated(host, &ready.name)?;
    secret::require_placeholder_present(host, &ready.name)?;

    let files = files::place_all(host, &ready.name, &config.files, files::Conflict::Refuse)?;
    identity::ensure(host, &ready.name, &config.git)?;
    tools::sandbox_ready(host, &ready.name)?;
    secret::configure_git_credential(host, &ready.name)?;

    repository::ensure_bare_clone(host, &ready.name, project, &layout)?;
    let branch =
        repository::resolve_start_ref(host, &ready.name, &layout, &paths, &mut project_metadata)?;
    let managed =
        repository::ensure_worktrees(host, &ready.name, &layout, &project_metadata, &branch)?;

    let worktrees = observed_worktrees(host, &ready.name, &layout, &project_metadata)?;
    let notes = tools::worktrees_ready(host, &ready.name, &layout, managed.len())?;

    Ok(PrepareOutput {
        project: project_metadata.display_id(),
        sandbox: ready.name,
        mode: project_metadata.provisioning.mode,
        start_ref: branch,
        sandbox_state: ready.state,
        worktrees,
        files,
        notes,
        already_built: false,
        warnings,
    })
}

/// 目標構成をすべて満たしたSandboxが既にあるか。
///
/// ある場合は副作用なしのno-op成功とする。判定はmetadataの完全性だけで済ませず、
/// Sandbox identityまで確認する。
fn already_built(
    host: &dyn HostEnvironment,
    paths: &ProjectPaths,
    name: &SandboxName,
    metadata: &ProjectMetadata,
    layout: &SandboxLayout,
    workspace_root: &Path,
) -> Result<Option<PrepareOutput>> {
    let _ = paths;
    let provisioning = &metadata.provisioning;
    if provisioning.start_ref.is_none() {
        return Ok(None);
    }

    let sandboxes = daemon::list(host)?;
    let Some(entry) = sandboxes
        .into_iter()
        .find(|entry| entry.name == name.as_str())
    else {
        return Ok(None);
    };

    sandbox::verify_identity(&entry, name, workspace_root)?;

    // 要求した本数が揃っているかは、Sandboxの中を見て決める。中を見られない場合は
    // 揃っているとは言えないため、通常の構築経路を通す。
    for name in layout.worktree_names(provisioning.requested_worktrees) {
        let path = format!("{}/{name}", layout.bare_root());
        if !sandbox::path_exists(host, &entry.name, &path)? {
            return Ok(None);
        }
    }

    let worktrees = observed_worktrees(host, &entry.name, layout, metadata)?;
    Ok(Some(PrepareOutput {
        project: metadata.display_id(),
        sandbox: entry.name,
        mode: provisioning.mode,
        start_ref: provisioning.start_ref.clone().unwrap_or_default(),
        sandbox_state: entry.state,
        worktrees,
        files: Vec::new(),
        notes: Vec::new(),
        already_built: true,
        warnings: Vec::new(),
    }))
}

/// 初回構築を完成させる世代を決める。
///
/// image buildの前にDockerfileが変わった場合は、現在のDockerfileを目標とする。
/// 既にimageがある場合は保存済み世代で完成させ、現在の内容は`rebuild`へ案内する。
fn adopt_generation(
    host: &dyn HostEnvironment,
    paths: &ProjectPaths,
    metadata: &mut ProjectMetadata,
    name: &SandboxName,
    current: &str,
    warnings: &mut Vec<Msg>,
) -> Result<String> {
    let stored = metadata.provisioning.dockerfile_sha256.clone();
    if current == stored {
        return Ok(stored);
    }

    if image::generation_is_built(host, name, &metadata.canonical_id, &stored)? {
        // 初回構築の途中へ別世代を混在させない。
        warnings.push(msg!(
            "warning-dockerfile-changed-during-build",
            project = metadata.display_id(),
            command = format!("sbxm rebuild {}", metadata.display_id())
        ));
        return Ok(stored);
    }

    metadata.provisioning.dockerfile_sha256 = current.to_string();
    metadata::update(paths, metadata)?;
    Ok(current.to_string())
}

/// metadataが宣言するmanaged worktreeの現在の状態。
fn observed_worktrees(
    host: &dyn HostEnvironment,
    sandbox: &str,
    layout: &SandboxLayout,
    metadata: &ProjectMetadata,
) -> Result<Vec<WorktreeRow>> {
    let provisioning = &metadata.provisioning;
    let names = layout.worktree_names(provisioning.requested_worktrees);
    let created_from = provisioning
        .start_ref
        .as_deref()
        .map(crate::git::origin_ref)
        .unwrap_or_default();
    let mut rows = Vec::with_capacity(names.len());
    for name in names {
        let path = format!("{}/{}", layout.bare_root(), name);
        // 停止中のSandboxではHEADを読めない。観測できない値を推測で埋めない。
        let outcome = sandbox::exec(host, sandbox, &["git", "-C", &path, "rev-parse", "HEAD"])?;
        let head = outcome
            .success()
            .then(|| outcome.stdout_text().trim().to_string())
            .filter(|head| !head.is_empty());
        rows.push(WorktreeRow {
            path: name,
            created_from: created_from.clone(),
            head,
            mode: provisioning.mode,
        });
    }
    Ok(rows)
}

/// 登録されていない案件は構築できない。
fn not_registered(project: &ProjectId) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::ProjectNotManaged,
            msg!("error-project-not-managed", project = project),
        )
        .remediation(msg!(
            "remediation-project-not-managed",
            command = format!("sbxm add {project}")
        )),
    )
}

/// 世代の切替中は構築を進めず、`rebuild`の完了を案内する。
fn require_no_rebuild(metadata: &ProjectMetadata) -> Result<()> {
    if metadata.rebuild.is_none() {
        return Ok(());
    }
    Err(Error::single(
        Diagnostic::new(
            ErrorId::RebuildIntentPending,
            msg!(
                "error-rebuild-intent-pending",
                project = metadata.display_id()
            ),
        )
        .remediation(msg!(
            "remediation-run-rebuild",
            command = format!("sbxm rebuild {}", metadata.display_id())
        )),
    ))
}

#[cfg(test)]
#[path = "prepare_test.rs"]
mod prepare_test;
