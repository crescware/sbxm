use std::path::PathBuf;

use crate::boundary::host::HostEnvironment;
use crate::design::ProgressSink;
use crate::diagnostics::{Msg, Result};
use crate::git;
use crate::metadata::ProjectMetadata;
use crate::msg;
use crate::project::ProjectId;
use crate::support::bundle;

use super::{PushRefusal, TagFollowing, push_to_sandbox, refresh_origin};

/// hostにあるrepositoryを`origin`とするSandboxで、`remote.origin.url`の書き出し。
///
/// Sandboxの中から届くremoteは無い。`git fetch origin`や`git push origin`は、この名前の
/// remote helperが無いため、何も変えずに失敗する。
const HOST_ORIGIN: &str = "sbxm-host::";

/// Sandboxのbare repositoryの`origin`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SandboxOrigin {
    /// GitHubのrepository。Sandboxからhttpsで取る。
    Github(ProjectId),
    /// hostにあるrepository。hostのgitが、ssh越しにSandboxのoriginを書き込む。
    ///
    /// Sandboxからhostへの経路は作らない。`project`は`remote.origin.url`に使う案件ID
    /// であり、hostの実pathはSandboxへ書かない。
    Host {
        repository: PathBuf,
        project: String,
    },
}

impl SandboxOrigin {
    pub fn of(metadata: &ProjectMetadata) -> Result<SandboxOrigin> {
        match metadata.repository.host_path() {
            None => Ok(SandboxOrigin::Github(ProjectId::parse(
                &metadata.display_id(),
            )?)),
            Some(repository) => Ok(SandboxOrigin::Host {
                repository: repository.to_path_buf(),
                project: metadata.display_id(),
            }),
        }
    }

    /// `remote.origin.url`へ書く値。
    pub fn url(&self) -> String {
        match self {
            SandboxOrigin::Github(project) => {
                git::https_remote_url(project.owner(), project.repository())
            }
            SandboxOrigin::Host { project, .. } => format!("{HOST_ORIGIN}{project}"),
        }
    }

    /// 観測した`remote.origin.url`がこのoriginを指すか。指さなければ、その理由。
    pub fn verify(&self, url: &str) -> std::result::Result<(), Msg> {
        match self {
            SandboxOrigin::Github(project) => {
                let canonical = project.canonical();
                match git::canonical_id_of_remote(url) {
                    Some(observed) if observed == canonical.as_str() => Ok(()),
                    Some(observed) => Err(msg!(
                        "cause-origin-elsewhere",
                        observed = observed,
                        declared = canonical
                    )),
                    None => Err(msg!("cause-origin-not-a-github-repository", observed = url)),
                }
            }
            SandboxOrigin::Host { .. } if url == self.url() => Ok(()),
            SandboxOrigin::Host { .. } => Err(msg!(
                "cause-origin-elsewhere",
                observed = url,
                declared = self.url()
            )),
        }
    }

    /// hostへ保存したbranchを、作り直したSandboxのbranchとして戻す。
    ///
    /// hostにあるrepositoryだけが、Sandboxから保存したbranchをhostに持つ。GitHubの案件では
    /// 何もしない。戻したbranchの名前を返す。
    pub fn restore_saved_branches(
        &self,
        host: &dyn HostEnvironment,
        sandbox: &str,
        git_dir: &str,
    ) -> Result<Vec<String>> {
        match self {
            SandboxOrigin::Github(_) => Ok(Vec::new()),
            SandboxOrigin::Host { repository, .. } => {
                bundle::restore_saved_branches(host, repository, sandbox, git_dir)
            }
        }
    }

    /// Sandboxのoriginを、今の状態にする。gitが断ったrefを返す。
    ///
    /// GitHubのrepositoryは、Sandboxの中で`git fetch --prune origin`を行う。hostにある
    /// repositoryは、hostのgitがそのbranchとtagをssh越しにSandboxのoriginへ書き込む。
    /// 送るbranchもtagも無いrepositoryは、何も書き込まずに断る。
    pub fn refresh(
        &self,
        host: &dyn HostEnvironment,
        sandbox: &str,
        git_dir: &str,
        progress: Option<&mut dyn ProgressSink>,
    ) -> Result<Vec<PushRefusal>> {
        match self {
            SandboxOrigin::Github(_) => {
                refresh_origin(host, sandbox, git_dir, TagFollowing::Auto, progress)?
                    .require_success()?;
                Ok(Vec::new())
            }
            SandboxOrigin::Host { repository, .. } => {
                bundle::require_something_to_send(host, repository)?;
                push_to_sandbox(host, repository, sandbox, git_dir)
            }
        }
    }
}
