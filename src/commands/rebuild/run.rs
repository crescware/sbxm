//! `sbxm rebuild`。
//!
//! 利用者が編集したDockerfileを新しい世代としてbuildし、保存されていない作業がない
//! ことを確かめてから、同じ目標構成でSandboxを作り直す。安全検査を省略するoptionは
//! 設けない。

use std::path::Path;

use crate::command::HostEnvironment;
use crate::compatibility::SandboxState;
use crate::config::GlobalConfig;
use crate::error::{Diagnostic, Error, ErrorId, Msg, Result};
use crate::metadata::{self, ProjectMetadata, RebuildIntent};
use crate::msg;
use crate::paths::ProjectPaths;
use crate::project::{ProjectId, SandboxLayout, SandboxName};

use crate::support::files::{self, Conflict};
use crate::support::image;
use crate::support::inventory::{self, Poll, ProjectState};
use crate::support::protection::{self, Unmanaged};
use crate::support::{
    daemon, generation, identity, repository, sandbox, secret, select, template, tools,
};

/// `rebuild`の結果。
#[derive(Debug, Clone)]
pub struct RebuildOutput {
    pub project: String,
    pub sandbox: String,
    /// 適用済みになったDockerfile hash。
    pub applied: String,
    /// 何も変更しなかったか。
    pub unchanged: bool,
    pub warnings: Vec<Msg>,
}

/// Dockerfileの変更をSandboxへ適用する。
pub fn run(
    config: &GlobalConfig,
    project: &ProjectId,
    host: &dyn HostEnvironment,
    workspace_root: &Path,
    poll: Poll,
) -> Result<RebuildOutput> {
    let canonical = project.canonical();
    let name = SandboxName::derive(&canonical);

    let mut locked = select::locked(config, project)?;
    let current = generation::current_dockerfile_hash(&locked.paths)?;
    // この案件のstateだけを、1回の一覧取得から決める。
    let entries = daemon::list(host)?;
    let state = inventory::state_of(&entries, &locked.metadata, workspace_root)?;

    let target = match &locked.metadata.rebuild {
        // intentがある場合は、intentに固定した世代だけを完成させる。
        Some(intent) => intent.target_dockerfile_sha256.clone(),
        None => {
            require_created(&locked.metadata, state, &name)?;
            if current == locked.metadata.provisioning.dockerfile_sha256 {
                return Ok(RebuildOutput {
                    project: locked.metadata.display_id(),
                    sandbox: name.as_str().to_string(),
                    applied: current,
                    unchanged: true,
                    warnings: Vec::new(),
                });
            }
            start_to_read_saved_state(
                host,
                &locked.metadata,
                &name,
                state == ProjectState::Stopped,
                workspace_root,
                poll,
            )?;
            let layout = SandboxLayout::new(&canonical);
            protection::inspect(
                host,
                name.as_str(),
                &layout,
                &locked.metadata,
                Unmanaged::Refused,
            )?;
            current.clone()
        }
    };

    let built = prepare_generation(
        host,
        &locked.paths,
        &name,
        &locked.metadata,
        &target,
        &current,
    )?;
    if locked.metadata.rebuild.is_none() {
        locked.metadata.rebuild = Some(RebuildIntent {
            target_dockerfile_sha256: target.clone(),
            previous_dockerfile_sha256: locked.metadata.provisioning.dockerfile_sha256.clone(),
        });
        metadata::update(&locked.paths, &locked.metadata)?;
    }

    let mut warnings = built.warnings;
    if current != target {
        warnings.push(msg!(
            "warning-dockerfile-changed-during-rebuild",
            project = locked.metadata.display_id(),
            command = format!("sbxm rebuild {}", locked.metadata.display_id())
        ));
    }

    let context = Switch {
        config,
        paths: &locked.paths,
        project,
        workspace_root,
        poll,
    };
    context.run(host, &name, &mut locked.metadata, &built.template)?;

    locked.metadata.provisioning.dockerfile_sha256 = target.clone();
    locked.metadata.rebuild = None;
    metadata::update(&locked.paths, &locked.metadata)?;

    Ok(RebuildOutput {
        project: locked.metadata.display_id(),
        sandbox: name.as_str().to_string(),
        applied: target,
        unchanged: false,
        warnings,
    })
}

/// 新世代のimage、archive、Template。
struct Generation {
    template: template::LoadedTemplate,
    warnings: Vec<Msg>,
}

/// target世代の成果物を用意する。
///
/// 現在のDockerfileがtarget世代である場合だけ生成でき、異なる場合は既存の成果物が
/// 揃っていることを条件とする。世代を混在させない。
fn prepare_generation(
    host: &dyn HostEnvironment,
    paths: &ProjectPaths,
    name: &SandboxName,
    metadata: &ProjectMetadata,
    target: &str,
    current: &str,
) -> Result<Generation> {
    if current != target && !image::generation_is_built(host, name, &metadata.canonical_id, target)?
    {
        // 固定済みtargetの成果物がなく、Dockerfileも別世代であるため再生成できない。
        return Err(Error::single(
            Diagnostic::new(
                ErrorId::RebuildGenerationMissing,
                msg!(
                    "error-rebuild-generation-missing",
                    project = metadata.display_id(),
                    target = target,
                    observed = current
                ),
            )
            .remediation(msg!(
                "remediation-rebuild-generation-missing",
                command = format!("sbxm destroy --force {}", metadata.display_id())
            )),
        ));
    }

    let built = image::ensure(
        host,
        name,
        &metadata.canonical_id,
        &paths.dockerfile(),
        target,
    )?;
    // 中断した再構築を続ける場合、成功済みの工程はinspectしてskipする。
    let template = match template::existing(host, &built)? {
        Some(template) => template,
        None => {
            let archive = image::ensure_archive(host, paths, &built, target)?;
            template::ensure(host, &archive, &built)?
        }
    };
    Ok(Generation {
        template,
        warnings: built.warnings,
    })
}

/// Sandboxの切り替えが最初から最後まで使う文脈。
///
/// 工程ごとに変わるのはSandbox名、metadata、新Templateだけである。
struct Switch<'a> {
    config: &'a GlobalConfig,
    paths: &'a ProjectPaths,
    project: &'a ProjectId,
    workspace_root: &'a Path,
    poll: Poll,
}

impl Switch<'_> {
    /// Sandboxを新世代へ切り替える。
    fn run(
        &self,
        host: &dyn HostEnvironment,
        name: &SandboxName,
        metadata: &mut ProjectMetadata,
        template: &template::LoadedTemplate,
    ) -> Result<()> {
        let Switch {
            config,
            paths,
            project,
            workspace_root,
            poll,
        } = *self;
        let layout = SandboxLayout::new(&metadata.canonical_id);

        // 新世代の準備には時間がかかる。切り替える対象は、その後の観測から決める。
        let entries = daemon::list(host)?;
        // Sandboxが不在の中断点からは、作成工程から続ける。
        //
        // 既にあるSandboxがどちらの世代のものかは問わない。一覧はTemplateを示さず、
        // 世代を観測する手段がないためである。既存のSandboxは、保存されていない作業が
        // ないことを確かめてから必ず作り直す。
        if let Some(entry) = inventory::single(&entries, name.as_str())? {
            start_to_read_saved_state(
                host,
                metadata,
                name,
                entry.state == SandboxState::Stopped,
                workspace_root,
                poll,
            )?;
            protection::inspect(host, name.as_str(), &layout, metadata, Unmanaged::Refused)?;
            // データ保護検査は上で済ませている。
            inventory::remove(host, name, poll)?;
        }

        // 再作成したSandboxは、`prepare`と同じ条件でGitHubへ届く必要がある。custom secretは
        // 作成時に結び付くため、作り直す前に確認する。
        secret::require_github(host, name.as_str())?;

        let ready = sandbox::ensure(host, name, template, workspace_root)?;

        secret::require_placeholder_present(host, &ready.name)?;

        identity::ensure(host, &ready.name, &config.git)?;
        tools::sandbox_ready(host, &ready.name)?;
        secret::configure_git_credential(host, &ready.name)?;
        files::place_all(host, &ready.name, &config.files, Conflict::Overwrite)?;
        repository::ensure_bare_clone(host, &ready.name, project, &layout)?;
        let branch = repository::resolve_start_ref(host, &ready.name, &layout, paths, metadata)?;
        repository::ensure_worktrees(host, &ready.name, &layout, metadata, &branch)?;
        sandbox::require_credentials_isolated(host, &ready.name)?;
        Ok(())
    }
}

/// 保存されていない作業を読むために、停止しているSandboxを起動する。
///
/// `rebuild`はこのSandboxをこれから作り直す。状態を読むためだけの起動を利用者へ
/// 求めない。
fn start_to_read_saved_state(
    host: &dyn HostEnvironment,
    metadata: &ProjectMetadata,
    name: &SandboxName,
    stopped: bool,
    workspace_root: &Path,
    poll: Poll,
) -> Result<()> {
    if !stopped {
        return Ok(());
    }
    inventory::start(host, name.as_str())?;
    inventory::wait_until_running(host, metadata, workspace_root, poll)?;
    Ok(())
}

/// `rebuild`は、Sandboxを持つ案件だけを対象とする。
fn require_created(
    metadata: &ProjectMetadata,
    state: ProjectState,
    name: &SandboxName,
) -> Result<()> {
    match state {
        ProjectState::Running | ProjectState::Stopped => Ok(()),
        ProjectState::NotCreated => Err(inventory::not_created(metadata, name.as_str())),
    }
}

#[cfg(test)]
#[path = "run_test.rs"]
mod run_test;

#[cfg(test)]
#[path = "resume_test.rs"]
mod resume_test;
