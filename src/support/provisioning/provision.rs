use std::path::Path;

use crate::boundary::host::HostEnvironment;
use crate::design::{Fact, ProgressSink, Warning};
use crate::diagnostics::Result;
use crate::msg;
use crate::paths;
use crate::project::SandboxName;

use crate::support::select::Locked;
use crate::support::{image, sandbox, template};

use super::{ExternalPreconditions, ProvisioningInputs, ProvisioningOutput, provision_interior};

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
    let generation = inputs.dockerfile_sha256.as_str();

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

    let archive = image::ensure_archive(host, &locked.paths, &built, generation, progress)?;
    let loaded = if let Some(loaded) = template::verified_existing(host, &built, archive.path())? {
        loaded
    } else {
        let outcome = template::ensure(host, archive.path(), &built, progress);
        archive.cleanup_after(outcome, &mut warnings, progress)?
    };

    let ready = sandbox::ensure(host, &name, &loaded, workspace_root, progress)?;
    if ready.workspace_restored {
        warnings.push(
            Warning::text(msg!(
                "warning-workspace-restored",
                sandbox = ready.name.clone()
            ))
            .fact(Fact::path(&paths::display(&ready.workspace)))
            .explain(msg!("guidance-workspace-restored")),
        );
    }
    provision_interior(locked, inputs, host, progress, warnings)
}
