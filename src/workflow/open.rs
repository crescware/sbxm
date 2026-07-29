//! `sbxm open`。
//!
//! 登録済み案件のSandboxを起動し、SSHでterminalを引き渡す。Sandboxを新規作成しない。

use std::path::Path;

use crate::command::{CommandSpec, HostEnvironment};
use crate::config::GlobalConfig;
use crate::error::{Diagnostic, Error, ErrorId, Result};
use crate::metadata::ProjectMetadata;
use crate::msg;
use crate::project::{ProjectId, SandboxLayout};

use super::inventory::{self, Poll, ProjectState};
use super::select::{self, ProjectPrompt};
use super::{daemon, rebuild, worktree};

/// 接続先と、接続前に見せる情報。
#[derive(Debug)]
pub struct Prepared {
    pub project: String,
    pub sandbox: String,
    /// 接続先のSSH host名。
    pub ssh_host: String,
    pub worktrees: Vec<String>,
}

/// SSHへ引き渡せる状態までSandboxを整える。
///
/// 1. 対象を引数またはpromptで解決する
/// 2. project lockを取得する
/// 3. Docker Engineへの疎通を確認する
/// 4. 1回の一覧取得からSandbox identityとstateを検証する
/// 5. runningでなければ起動して待つ
/// 6. hostのSSH Agentが届かないことをSandboxの中から確認する
/// 7. managed worktreeをmetadataとGitから検証する
///
/// lockはこの関数のあいだだけ保持する。SSH sessionそのものはsbxmのmutationではなく、
/// 接続中に別terminalの`stop`が待たされる状態を作らない。
pub fn prepare(
    config: &GlobalConfig,
    requested: Option<&ProjectId>,
    host: &dyn HostEnvironment,
    prompt: &mut dyn ProjectPrompt,
    workspace_root: &Path,
    poll: Poll,
) -> Result<Prepared> {
    // 対象が決まる前にhostの状態へ触れない。
    let candidate = select::one(config, requested, prompt)?;

    let _lock = candidate.paths.acquire_lock()?;

    // lockを取る前に読んだmetadataは古くなり得る。判定はlock後の内容だけで行う。
    let metadata = candidate.reload()?;
    let name = metadata.sandbox_name();
    rebuild::require_no_rebuild(&metadata)?;

    require_docker(host)?;

    let entries = daemon::list(host)?;
    match inventory::state_of(&entries, &metadata, workspace_root)? {
        ProjectState::Running => {}
        ProjectState::Stopped => inventory::start(host, name.as_str())?,
        ProjectState::NotCreated => return Err(inventory::not_created(&metadata, name.as_str())),
    }
    inventory::wait_until_running(host, &metadata, workspace_root, poll)?;

    // 接続する前に、hostのSSH Agentが届かないことを中から確かめる。
    crate::workflow::sandbox::require_credentials_isolated(host, name.as_str())?;

    let layout = SandboxLayout::new(&metadata.canonical_id);
    let worktrees = verify_worktrees(host, name.as_str(), &layout, &metadata)?;

    Ok(Prepared {
        project: metadata.display_id(),
        sandbox: name.as_str().to_string(),
        ssh_host: format!("{name}.sbx"),
        worktrees,
    })
}

/// terminalをSSHへ引き渡す。
///
/// SSHのexit statusが0なら成功とし、非ゼロは理由を推測せず外部command失敗とする。
pub fn connect(host: &dyn HostEnvironment, prepared: &Prepared) -> Result<()> {
    let spec = CommandSpec::inherit("ssh", &[&prepared.ssh_host]);
    host.run(&spec)?.require_success()?;
    Ok(())
}

/// Docker Engineへ疎通できることを確認する。
fn require_docker(host: &dyn HostEnvironment) -> Result<()> {
    let spec = CommandSpec::probe("docker", &["version", "--format", "{{.Server.Version}}"]);
    let outcome = host.run(&spec)?;
    if outcome.success() && !outcome.stdout_text().trim().is_empty() {
        return Ok(());
    }
    Err(Error::single(
        Diagnostic::new(
            ErrorId::DockerUnreachable,
            msg!(
                "error-docker-unreachable",
                detail = "the server version could not be read"
            ),
        )
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
                    msg!(
                        "error-sandbox-repository-unusable",
                        path = path,
                        detail =
                            "the project asks for this managed worktree, but Git does not have it"
                    ),
                )
                .remediation(msg!("remediation-sandbox-repository-unusable", path = path)),
            ));
        }
        present.push(format!("{bare_root}/{name}"));
    }
    Ok(present)
}

#[cfg(test)]
#[path = "open_test.rs"]
mod open_test;
