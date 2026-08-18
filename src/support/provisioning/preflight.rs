use crate::command::HostEnvironment;
use crate::config::GlobalConfig;
use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;
use crate::paths;
use crate::project::{ProjectId, SandboxLayout, SandboxName};
use std::path::Path;

use crate::support::select::Locked;
use crate::support::{docker, files, identity, image, repository, sandbox, secret, tools};

/// repair計画を返す前に、既存成果物のidentityと衝突をread-onlyで確認する。
///
/// ここではmissingな値を補わない。repair実行で作成・設定してよいものと、既存の別物を
/// 上書きしてはいけないものを先に分けることで、intent保存後に不一致へ当たらないように
/// する。
pub(crate) fn preflight(
    locked: &Locked,
    config: &GlobalConfig,
    target: &str,
    host: &dyn HostEnvironment,
    workspace_root: &Path,
    has_sandbox: bool,
    require_workspace: bool,
) -> Result<()> {
    let name = SandboxName::derive(locked.metadata.canonical_id());
    let project = ProjectId::parse(&locked.metadata.display_id())?;
    let layout = SandboxLayout::new(locked.metadata.canonical_id());

    secret::require_github(host, name.as_str())?;
    docker::require_reachable(host)?;
    image::verify_generation(host, &name, locked.metadata.canonical_id(), target)?;

    if !has_sandbox {
        return Ok(());
    }

    if require_workspace && !sandbox::workspace_exists(workspace_root, &name)? {
        let workspace = sandbox::workspace_path(workspace_root, &name);
        return Err(Error::single(
            Diagnostic::new(
                ErrorId::InitialProvisioningIncomplete,
                msg!(
                    "error-initial-provisioning-incomplete",
                    project = locked.metadata.display_id()
                ),
            )
            .fact(Fact::path(&paths::display(&workspace)))
            .remediation(msg!("remediation-initial-provisioning-incomplete")),
        ));
    }

    sandbox::require_credentials_isolated(host, name.as_str())?;
    secret::require_placeholder_present(host, name.as_str())?;
    files::preflight(host, name.as_str(), &config.files)?;
    identity::verify(host, name.as_str(), &locked.metadata.git_identity)?;

    let installed = tools::Installed::observe(host, name.as_str())?;
    if installed.has(&tools::Gh) {
        identity::verify_git_protocol(host, name.as_str())?;
    }
    secret::verify_git_credential(host, name.as_str())?;
    repository::verify_existing(host, name.as_str(), &project, &layout, &locked.metadata)?;
    Ok(())
}
