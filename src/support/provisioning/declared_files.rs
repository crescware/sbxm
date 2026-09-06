use crate::boundary::host::HostEnvironment;
use crate::config::GlobalConfig;
use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::metadata::ProjectMetadata;
use crate::msg;
use crate::support::files::{self, PlacedFile};

/// 宣言fileの現在状態を、復旧すべきbaselineと照らして観測する。
///
/// 対象はintentが固定したsnapshot、それが無ければ完成時に残したbaselineであり、
/// 現在のglobal configそのものではない。completed projectへ後から宣言を足しても、
/// この観測はそれを欠落として報告しない。新しい宣言の配置は`apply`の責務である。
///
/// baselineが無い案件（この機能より前に完成した案件）は、現在の宣言と実際に一致する
/// 場合だけ健全とみなす。一致しない場合、それが本当に壊れているのか、後から宣言が
/// 増えただけなのかをこの観測だけでは一意に決められないため拒否する。
pub(crate) fn declared_files(
    host: &dyn HostEnvironment,
    sandbox: &str,
    metadata: &ProjectMetadata,
    config: &GlobalConfig,
) -> Result<Vec<PlacedFile>> {
    if let Some(intent) = &metadata.initial_provisioning {
        return files::observe_against_baseline(host, sandbox, &intent.files);
    }
    if let Some(baseline) = &metadata.declared_files {
        return files::observe_against_baseline(host, sandbox, baseline);
    }
    let observed = files::observe(host, sandbox, &config.files)?;
    if observed
        .iter()
        .any(|file| file.placement != files::Placement::Unchanged)
    {
        return Err(baseline_ambiguous(metadata));
    }
    Ok(observed)
}

fn baseline_ambiguous(metadata: &ProjectMetadata) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::InitialProvisioningBaselineAmbiguous,
            msg!(
                "error-initial-provisioning-baseline-ambiguous",
                project = metadata.display_id()
            ),
        )
        .fact(Fact::reason(msg!(
            "cause-initial-provisioning-baseline-ambiguous"
        )))
        .remediation(msg!("remediation-initial-provisioning-baseline-ambiguous")),
    )
}
