use std::path::PathBuf;

use crate::boundary::host::HostEnvironment;
use crate::diagnostics::{Msg, Result};
use crate::git;
use crate::metadata::ProjectMetadata;
use crate::msg;
use crate::paths::ProjectPaths;
use crate::project::{ProjectId, SandboxLayout};
use crate::repository::Provider;
use crate::support::bundle;

/// Sandboxのbare repositoryが`origin`として読むもの。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SandboxOrigin {
    /// GitHubのrepository。Sandboxからhttpsで取る。
    Github(ProjectId),
    /// hostにあるrepository。hostから送ったbundleを、Sandboxの中のfileとして読む。
    ///
    /// Sandboxからhostへの経路は作らない。送るbundleは案件の`staging`で作る。
    Host {
        repository: PathBuf,
        bundle: String,
        staging: PathBuf,
    },
}

impl SandboxOrigin {
    pub fn of(paths: &ProjectPaths, metadata: &ProjectMetadata) -> Result<SandboxOrigin> {
        match metadata.repository.provider() {
            Provider::Github => Ok(SandboxOrigin::Github(ProjectId::parse(
                &metadata.display_id(),
            )?)),
            Provider::Local => Ok(SandboxOrigin::Host {
                repository: PathBuf::from(metadata.repository.clone_url()),
                bundle: SandboxLayout::new(metadata.canonical_id()).origin_bundle(),
                staging: paths.bundles_dir(),
            }),
        }
    }

    /// `remote.origin.url`へ書く値。
    pub fn url(&self) -> String {
        match self {
            SandboxOrigin::Github(project) => {
                git::https_remote_url(project.owner(), project.repository())
            }
            SandboxOrigin::Host { bundle, .. } => bundle.clone(),
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
            SandboxOrigin::Host { bundle, .. } if url == bundle => Ok(()),
            SandboxOrigin::Host { bundle, .. } => Err(msg!(
                "cause-origin-elsewhere",
                observed = url,
                declared = bundle
            )),
        }
    }

    /// fetchの前に、originが読むものを用意する。
    ///
    /// hostにあるrepositoryは、そのbranchとtagをbundleにして送る。GitHubのrepository
    /// にはSandboxから取りに行くため、何もしない。
    pub fn deliver(&self, host: &dyn HostEnvironment, sandbox: &str) -> Result<()> {
        match self {
            SandboxOrigin::Github(_) => Ok(()),
            SandboxOrigin::Host {
                repository,
                bundle,
                staging,
            } => bundle::send_to_sandbox(
                host,
                repository,
                &["--branches", "--tags"],
                staging,
                sandbox,
                bundle,
            ),
        }
    }
}
