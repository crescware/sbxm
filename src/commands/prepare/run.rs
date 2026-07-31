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
use crate::error::Result;
use crate::metadata::{self, CreationMode, ProjectMetadata};
use crate::msg;
use crate::paths::ProjectPaths;
use crate::project::{ProjectId, SandboxLayout, SandboxName};

use crate::support::files::PlacedFile;
use crate::support::tools::Note;
use crate::support::{
    daemon, files, generation, identity, image, repository, sandbox, secret, select, template,
    tools,
};
use crate::ui::{ProgressSink, Warning};

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
    pub warnings: Vec<Warning>,
}

/// 登録済み案件のSandboxを構築する。
pub fn run(
    config: &GlobalConfig,
    project: &ProjectId,
    host: &dyn HostEnvironment,
    workspace_root: &Path,
    progress: &mut dyn ProgressSink,
) -> Result<PrepareOutput> {
    let canonical = project.canonical();
    let name = SandboxName::derive(&canonical);

    let mut locked = select::locked(config, project)?;
    generation::require_no_rebuild(&locked.metadata)?;

    let layout = SandboxLayout::new(&canonical);
    let mut warnings = Vec::new();

    if let Some(output) = already_built(
        host,
        &locked.paths,
        &name,
        &locked.metadata,
        &layout,
        workspace_root,
    )? {
        return Ok(output);
    }

    // custom secretはSandboxの作成時に結び付く。あとから登録しても既存のSandboxには
    // 届かないため、作成より前に、そしてimageを組む前に確認する。
    secret::require_github(host, name.as_str())?;

    let current = generation::current_dockerfile_hash(&locked.paths)?;
    let generation = adopt_generation(
        host,
        &locked.paths,
        &mut locked.metadata,
        &name,
        &current,
        &mut warnings,
    )?;

    let built = image::ensure(
        host,
        &name,
        locked.metadata.canonical_id(),
        &locked.paths.dockerfile(),
        &generation,
        progress,
    )?;
    warnings.extend(built.warnings.clone());
    let archive = image::ensure_archive(host, &locked.paths, &built, &generation, progress)?;
    let loaded = template::ensure(host, &archive, &built, progress)?;

    let ready = sandbox::ensure(host, &name, &loaded, workspace_root, progress)?;
    // hostのSSH Agentが届かないことを、daemonの起動条件から推定せず中から確かめる。
    sandbox::require_credentials_isolated(host, &ready.name)?;
    secret::require_placeholder_present(host, &ready.name)?;

    let files = files::place_all(host, &ready.name, &config.files, files::Conflict::Refuse)?;
    identity::ensure(host, &ready.name, &config.git)?;
    tools::sandbox_ready(host, &ready.name)?;
    secret::configure_git_credential(host, &ready.name)?;

    repository::ensure_bare_clone(host, &ready.name, project, &layout, progress)?;
    let branch = repository::resolve_start_ref(
        host,
        &ready.name,
        &layout,
        &locked.paths,
        &mut locked.metadata,
    )?;
    let managed = repository::ensure_worktrees(
        host,
        &ready.name,
        &layout,
        &locked.metadata,
        &branch,
        progress,
    )?;

    let worktrees = observed_worktrees(host, &ready.name, &layout, &locked.metadata)?;
    let notes = tools::worktrees_ready(host, &ready.name, &layout, managed.len())?;

    Ok(PrepareOutput {
        project: locked.metadata.display_id(),
        sandbox: ready.name,
        mode: locked.metadata.provisioning.mode,
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
    warnings: &mut Vec<Warning>,
) -> Result<String> {
    let stored = metadata.provisioning.dockerfile_sha256.clone();
    if current == stored {
        return Ok(stored);
    }

    if image::generation_is_built(host, name, metadata.canonical_id(), &stored)? {
        // 注意だけを出して終えない。現在のDockerfileを適用する手順まで示す。
        warnings.push(
            Warning::text(msg!(
                "warning-dockerfile-changed-during-build",
                project = metadata.display_id()
            ))
            .explain(msg!("guidance-apply-current-dockerfile"))
            .try_run(format!("sbxm rebuild {}", metadata.display_id())),
        );
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

#[cfg(test)]
#[path = "run_test.rs"]
mod run_test;

#[cfg(test)]
#[path = "resume_test.rs"]
mod resume_test;

#[cfg(test)]
#[path = "generation_test.rs"]
mod generation_test;

#[cfg(test)]
#[path = "worktree_test.rs"]
mod worktree_test;

#[cfg(test)]
#[path = "secret_test.rs"]
mod secret_test;

#[cfg(test)]
#[path = "tools_test.rs"]
mod tools_test;

#[cfg(test)]
#[path = "output_test.rs"]
mod output_test;
