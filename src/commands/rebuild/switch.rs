use std::path::Path;

use crate::command::HostEnvironment;
use crate::compatibility::SandboxState;
use crate::config::GlobalConfig;
use crate::diagnostics::Result;
use crate::metadata::ProjectMetadata;
use crate::paths::ProjectPaths;
use crate::project::{ProjectId, SandboxLayout, SandboxName};

use crate::design::ProgressSink;
use crate::support::files::{self, Conflict};
use crate::support::inventory::{self, Poll};
use crate::support::protection::{self, Unmanaged};
use crate::support::{daemon, disk, identity, repository, sandbox, secret, template, tools};

use super::start_to_read_saved_state;

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
    pub(crate) fn run(
        &self,
        host: &dyn HostEnvironment,
        name: &SandboxName,
        metadata: &mut ProjectMetadata,
        template: &template::LoadedTemplate,
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
                progress,
            )?;
            protection::inspect(host, name.as_str(), &layout, metadata, Unmanaged::Refused)?;
            // データ保護検査は上で済ませている。
            inventory::remove(host, name, poll, progress)?;
        }

        // 再作成したSandboxは、`prepare`と同じ条件でGitHubへ届く必要がある。custom secretは
        // 作成時に結び付くため、作り直す前に確認する。
        secret::require_github(host, name.as_str())?;

        let ready = sandbox::ensure(host, name, template, workspace_root, progress)?;

        secret::require_placeholder_present(host, &ready.name)?;

        // sbxm自身がSandbox内を変更する工程が失敗した場合だけ、失敗直後の空き容量を
        // 追加のfactとして載せる。平常時はcommandを1つも増やさない。
        let decorate = |error| disk::attach_on_failure(host, &ready.name, ready.state, error);

        identity::ensure(host, &ready.name, &metadata.git_identity).map_err(decorate)?;
        tools::SandboxReady::announce(host, &ready.name)?;
        secret::configure_git_credential(host, &ready.name).map_err(decorate)?;
        files::place_all(host, &ready.name, &config.files, Conflict::Overwrite)
            .map_err(decorate)?;
        repository::ensure_bare_clone(host, &ready.name, project, &layout, progress)
            .map_err(decorate)?;
        let branch = repository::resolve_start_ref(host, &ready.name, &layout, paths, metadata)?;
        repository::ensure_worktrees(host, &ready.name, &layout, metadata, &branch, progress)
            .map_err(decorate)?;
        sandbox::require_credentials_isolated(host, &ready.name)?;
        Ok(())
    }
}
