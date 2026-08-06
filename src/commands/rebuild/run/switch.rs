use std::path::Path;

use crate::command::HostEnvironment;
use crate::config::GlobalConfig;
use crate::diagnostics::Result;
use crate::metadata::ProjectMetadata;
use crate::paths::ProjectPaths;
use crate::project::{ProjectId, SandboxLayout, SandboxName};

use crate::design::ProgressSink;
use crate::support::files::{self, Conflict};
use crate::support::inventory::{self, Poll};
use crate::support::protection::ProtectionPermit;
use crate::support::{identity, repository, sandbox, secret, template, tools};

/// Sandboxの切り替えが最初から最後まで使う文脈。
///
/// 工程ごとに変わるのはSandbox名、metadata、新Templateだけである。
pub(super) struct Switch<'a> {
    pub(super) config: &'a GlobalConfig,
    pub(super) paths: &'a ProjectPaths,
    pub(super) project: &'a ProjectId,
    pub(super) workspace_root: &'a Path,
    pub(super) poll: Poll,
}

impl Switch<'_> {
    /// Sandboxを新世代へ切り替える。
    ///
    /// `permit`は呼び出し側の`gate::authorize`が発行した、この1回のremoveだけに使う
    /// 許可証である。`existed`が示す通り、そもそも削除するSandboxが無い場合は使わず
    /// 破棄する。
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn run(
        &self,
        host: &dyn HostEnvironment,
        name: &SandboxName,
        metadata: &mut ProjectMetadata,
        template: &template::LoadedTemplate,
        existed: bool,
        permit: ProtectionPermit,
        progress: &mut dyn ProgressSink,
    ) -> Result<()> {
        let Switch {
            config,
            paths,
            project,
            workspace_root,
            poll,
        } = *self;
        let layout = SandboxLayout::new(metadata.canonical_id());

        if existed {
            // rebuildに`--force`は無く、常にsbx自身の確認とactive-session検査を経る。
            inventory::remove_protected(host, name, permit, poll, progress)?;
        }

        // 再作成したSandboxは、`prepare`と同じ条件でGitHubへ届く必要がある。custom secretは
        // 作成時に結び付くため、作り直す前に確認する。
        secret::require_github(host, name.as_str())?;

        let ready = sandbox::ensure(host, name, template, workspace_root, progress)?;

        secret::require_placeholder_present(host, &ready.name)?;

        identity::ensure(host, &ready.name, &metadata.git_identity)?;
        tools::SandboxReady::announce(host, &ready.name)?;
        secret::configure_git_credential(host, &ready.name)?;
        files::place_all(host, &ready.name, &config.files, Conflict::Overwrite)?;
        repository::ensure_bare_clone(host, &ready.name, project, &layout, progress)?;
        let branch = repository::resolve_start_ref(host, &ready.name, &layout, paths, metadata)?;
        repository::ensure_worktrees(host, &ready.name, &layout, metadata, &branch, progress)?;
        sandbox::require_credentials_isolated(host, &ready.name)?;
        Ok(())
    }
}
