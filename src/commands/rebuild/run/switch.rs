use std::path::Path;

use crate::boundary::host::HostEnvironment;
use crate::config::GlobalConfig;
use crate::diagnostics::Result;
use crate::metadata::ProjectMetadata;
use crate::paths::ProjectPaths;
use crate::project::{ProjectId, SandboxLayout, SandboxName};

use crate::design::ProgressSink;
use crate::support::files::{self, Conflict};
use crate::support::inventory::{self, Poll};
use crate::support::protection::ProtectionPermit;
use crate::support::{disk, identity, provisioning, repository, sandbox, secret, template, tools};

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
    /// 破棄する。hostへ保存したbranchを戻した場合は、その名前を返す。
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
    ) -> Result<Vec<String>> {
        let Switch {
            config,
            paths,
            project,
            workspace_root,
            poll,
        } = *self;
        let layout = SandboxLayout::new(metadata.canonical_id());
        // 宣言fileは古いSandboxを消す前に読む。読めないfileがあれば、何も消さずに止まる。
        let inputs = provisioning::ProvisioningInputs::capture_files(paths, config)?;

        if existed {
            // rebuildに`--force`は無く、常にsbx自身の確認とactive-session検査を経る。
            // 削除する対象は許可証が持つ。`name`は再作成の側だけが使う。
            inventory::remove_protected(host, permit, poll, progress)?;
        }

        // 再作成したSandboxは、`prepare`と同じ条件でGitHubへ届く必要がある。tokenの
        // ないままimageを組み直してSandboxを作らないよう、作り直す前に確認する。
        // hostにあるrepositoryはhostから送るため、tokenを使わない。
        let origin = repository::SandboxOrigin::of(paths, metadata)?;
        let registration = match origin {
            repository::SandboxOrigin::Github(_) => {
                Some(secret::require_github(host, name.as_str())?)
            }
            repository::SandboxOrigin::Host { .. } => None,
        };

        let ready = sandbox::ensure(host, name, template, workspace_root, progress)?;

        // sbxm自身がSandbox内を変更する工程が失敗した場合だけ、失敗直後の空き容量を
        // 追加のfactとして載せる。平常時はcommandを1つも増やさない。
        let decorate = |error| disk::attach_on_failure(host, &ready.name, ready.state, error);

        identity::ensure(host, &ready.name, &metadata.git_identity).map_err(decorate)?;
        tools::SandboxReady::announce(host, &ready.name).map_err(decorate)?;
        if let Some(registration) = &registration {
            secret::configure_git_credential(host, &ready.name, registration.placeholder())
                .map_err(decorate)?;
            secret::configure_token_env(host, &ready.name, registration.placeholder())
                .map_err(decorate)?;
            secret::require_github_accepts(host, &ready.name, project, registration)?;
        }
        let declarations: Vec<_> = inputs
            .iter()
            .map(|input| input.declaration.clone())
            .collect();
        files::place_all(host, &ready.name, &declarations, Conflict::Overwrite)
            .map_err(decorate)?;
        // 作り直したSandboxへ置いた内容が、以後の`apply`が置き換えてよい基準になる。
        metadata.declared_files = Some(provisioning::recorded_files(&inputs));
        repository::ensure_bare_clone(host, &ready.name, &origin, &layout, progress)
            .map_err(decorate)?;
        // hostにあるrepositoryは、hostへ保存したbranchをworktreeより先に戻す。起点branchも
        // 戻したものがあれば、その先端からworktreeを作り直す。
        let restored = origin
            .restore_saved_branches(host, ready.name.as_str(), &layout.bare_git_dir())
            .map_err(decorate)?;
        let branch = repository::resolve_start_ref(host, &ready.name, &layout, paths, metadata)?;
        repository::ensure_worktrees(
            host,
            &ready.name,
            &layout,
            metadata,
            &branch,
            &restored,
            progress,
        )
        .map_err(decorate)?;
        sandbox::require_credentials_isolated(host, &ready.name)?;
        Ok(restored)
    }
}
