use std::path::Path;

use crate::boundary::host::HostEnvironment;
use crate::design::{Fact, ProgressSink, Warning};
use crate::diagnostics::Result;
use crate::msg;
use crate::paths;
use crate::project::{ProjectId, SandboxLayout, SandboxName};

use crate::support::select::Locked;
use crate::support::{disk, files, identity, image, repository, sandbox, secret, template, tools};

use super::observed_worktrees::observed_worktrees;
use super::{ExternalPreconditions, ProvisioningInputs, ProvisioningOutput};

/// 固定済みgenerationへ向けて初回構築を進める唯一の共有境界。
///
/// secretとengineのread-only事前条件は`preconditions`が既に確認済みであることを
/// 証明する。呼び出しごとに1回だけ確認すればよいよう、ここでは同じ外部callを
/// 再発行しない。Dockerfileと宣言fileは`inputs`が固定したsnapshotだけを読み、
/// 生きているhost pathへは二度と触れない。
#[allow(clippy::too_many_arguments)]
pub(crate) fn provision(
    locked: &mut Locked,
    inputs: &ProvisioningInputs,
    _preconditions: ExternalPreconditions,
    host: &dyn HostEnvironment,
    workspace_root: &Path,
    progress: &mut dyn ProgressSink,
    mut warnings: Vec<Warning>,
) -> Result<ProvisioningOutput> {
    let canonical = locked.metadata.canonical_id().clone();
    let name = SandboxName::derive(&canonical);
    let project = ProjectId::parse(&locked.metadata.display_id())?;
    let layout = SandboxLayout::new(&canonical);
    let generation = inputs.dockerfile_sha256.as_str();

    // buildへ渡す直前にもう一度、snapshotが作った時点のままであることを確かめる。
    inputs.verify_unchanged()?;
    let built = image::ensure(
        host,
        &name,
        locked.metadata.canonical_id(),
        &inputs.dockerfile_path,
        generation,
        progress,
    )?;
    warnings.extend(built.warnings.clone());

    // Templateの再利用は、名前一致だけでなく、runtimeのidがlabel検証済みhost imageと
    // 対応することまで確かめてから許可する。archiveのconfig digestを期待値として使う
    // ため、再利用する場合でも一度archiveを作る。
    let archive = image::ensure_archive(host, &locked.paths, &built, generation, progress)?;
    let loaded = if let Some(loaded) = template::verified_existing(host, &built, archive.path())? {
        loaded
    } else {
        let outcome = template::ensure(host, archive.path(), &built, progress);
        archive.cleanup_after(outcome, &mut warnings, progress)?
    };

    let ready = sandbox::ensure(host, &name, &loaded, workspace_root, progress)?;
    if ready.workspace_restored {
        // 消えていたmount点を作り直したことを、成功のなかへ黙って混ぜない。対象のpathと
        // 変更範囲を示し、Sandboxの中には触れていないことまで告げる。
        warnings.push(
            Warning::text(msg!(
                "warning-workspace-restored",
                sandbox = ready.name.clone()
            ))
            .fact(Fact::path(&paths::display(&ready.workspace)))
            .explain(msg!("guidance-workspace-restored")),
        );
    }
    // hostのSSH Agentが届かないことを、daemonの起動条件から推定せず中から確かめる。
    sandbox::require_credentials_isolated(host, &ready.name)?;
    secret::require_placeholder_present(host, &ready.name)?;

    // sbxm自身がSandbox内を変更する工程が失敗した場合だけ、失敗直後の空き容量を
    // 追加のfactとして載せる。平常時はcommandを1つも増やさない。
    let decorate = |error| disk::attach_on_failure(host, &ready.name, ready.state, error);

    // copyへ渡す直前にもう一度、snapshotが作った時点のままであることを確かめる。
    inputs.verify_unchanged()?;
    let placed_files = files::place_all(
        host,
        &ready.name,
        &inputs.file_declarations(),
        files::Conflict::Refuse,
    )
    .map_err(decorate)?;
    identity::ensure(host, &ready.name, &locked.metadata.git_identity).map_err(decorate)?;
    tools::SandboxReady::announce(host, &ready.name).map_err(decorate)?;
    secret::configure_git_credential(host, &ready.name).map_err(decorate)?;

    repository::ensure_bare_clone(host, &ready.name, &project, &layout, progress)
        .map_err(decorate)?;
    let branch = repository::resolve_start_ref(
        host,
        &ready.name,
        &layout,
        &locked.paths,
        &mut locked.metadata,
    )?;
    repository::ensure_worktrees(
        host,
        &ready.name,
        &layout,
        &locked.metadata,
        &branch,
        progress,
    )
    .map_err(decorate)?;

    // ensure_worktreesは各工程のpost-conditionを検査し、ここでは最終表示用の観測を行う。
    let worktrees = observed_worktrees(host, &ready.name, &layout, &locked.metadata)?;
    Ok(ProvisioningOutput {
        project: locked.metadata.display_id(),
        sandbox: ready.name,
        mode: locked.metadata.provisioning.mode,
        start_ref: branch,
        sandbox_state: ready.state,
        worktrees,
        files: placed_files,
        already_built: false,
        warnings,
    })
}
