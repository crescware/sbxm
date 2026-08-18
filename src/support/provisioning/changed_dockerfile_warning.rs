use crate::design::Warning;
use crate::metadata::ProjectMetadata;
use crate::msg;

/// generation選択時の警告を作る。
pub(crate) fn changed_dockerfile_warning(metadata: &ProjectMetadata) -> Warning {
    Warning::text(msg!(
        "warning-dockerfile-changed-during-build",
        project = metadata.display_id()
    ))
    .explain(msg!("guidance-apply-current-dockerfile"))
    .try_run(format!("sbxm rebuild {}", metadata.display_id()))
}
