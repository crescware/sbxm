use std::path::Path;

use crate::boundary::host::HostEnvironment;
use crate::config::GlobalConfig;
use crate::design::ProgressSink;
use crate::diagnostics::{Error, Result};
use crate::metadata;
use crate::msg;
use crate::support::image;
use crate::support::select::Locked;

use super::{
    ProvisioningInputs, ProvisioningOutput, ProvisioningState, initial_intent, observe, provision,
    require_repair, verify_external_preconditions,
};

/// 固定済みintentのもとで初回構築を完了させる、唯一の共有workflow。
///
/// 入口commandはこの順序を持たない。preflight → 入力snapshotの固定 → intentの保存 →
/// snapshotだけを入力にした構築 → 完成の再観測 → intentのclearとbaselineの記録、という
/// 順序をここだけが持ち、どの入口から入っても同じ契約を通る。
pub(crate) fn build_initial(
    locked: &mut Locked,
    config: &GlobalConfig,
    host: &dyn HostEnvironment,
    workspace_root: &Path,
    progress: &mut dyn ProgressSink,
) -> Result<ProvisioningOutput> {
    let name = locked.metadata.sandbox_name();

    // custom secretはSandboxの作成時に結び付く。あとから登録しても既存のSandboxには
    // 届かないため、作成より前に、そしてimageを組む前に確認する。Dockerの到達性も
    // ここで一度だけ確認し、以降の`provision`の中では再確認しない。
    let preconditions = verify_external_preconditions(host, &name)?;

    // Dockerfileと宣言fileを1回だけ読み、privateなsnapshotへ複製する。以降はこの
    // snapshotだけを使い、生きているhost pathを二度と読まない。
    let inputs = ProvisioningInputs::capture(&locked.paths, config, None)?;

    // metadataのintentとtarget generationを、最初のhost側mutationより先にatomicに保存する。
    locked.metadata.initial_provisioning = Some(initial_intent(&inputs));
    locked
        .metadata
        .provisioning
        .dockerfile_sha256
        .clone_from(&inputs.dockerfile_sha256);
    metadata::update(&locked.paths, &locked.metadata)?;

    let project = locked.metadata.display_id();
    let warnings = image::cleanup_stale_archives(&locked.paths)
        .map_err(|error| with_open_command(error, &project))?;

    let output = provision(
        locked,
        &inputs,
        preconditions,
        host,
        workspace_root,
        progress,
        warnings,
    )
    .map_err(|error| with_open_command(error, &project))?;

    // 成果物をread-onlyで再確認できてからintentをclearする。clearのatomic replaceに失敗
    // した場合も、disk上のintentは残るため、次回のopenへ安全に渡る。
    let completed = observe(
        host,
        &locked.paths,
        config,
        &locked.metadata,
        workspace_root,
    )
    .map_err(|error| with_open_command(error, &project))?;
    completed
        .require_safe()
        .map_err(|error| with_open_command(error, &project))?;
    if !completed.is_complete() {
        return Err(require_repair(&locked.metadata, ProvisioningState::Pending));
    }
    locked.metadata.initial_provisioning = None;
    locked.metadata.declared_files = Some(initial_intent(&inputs).files);
    metadata::update(&locked.paths, &locked.metadata)
        .map_err(|error| with_open_command(error, &project))?;
    Ok(output)
}

/// intent保存後の失敗へ、次に実行できる明示的な復旧commandを足す。
fn with_open_command(error: Error, project: &str) -> Error {
    let Error::Diagnostics(mut diagnostics) = error else {
        return error;
    };
    if let Some(diagnostic) = diagnostics.last_mut() {
        let remediation = diagnostic
            .remediation
            .take()
            .unwrap_or_default()
            .explain(msg!("remediation-run-open"))
            .try_run(format!("sbxm open {project}"));
        diagnostic.remediation = Some(remediation);
    }
    Error::Diagnostics(diagnostics)
}
