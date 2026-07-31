use crate::diagnostics::{Error, ErrorId, Result};
use crate::metadata::{CreationMode, ProjectMetadata};
use crate::msg;
use crate::paths::{self, ProjectPaths};

/// 元の目標構成を、新規登録として再現するcommand。
///
/// 起点branchのないdetached modeは再現できない。案内できない構成を、実行すると
/// 別の結果になるcommandとして見せない。
pub(super) fn re_register(paths: &ProjectPaths, metadata: &ProjectMetadata) -> Result<String> {
    let provisioning = &metadata.provisioning;
    // 登録時と同じclone URLを示す。transportを暗黙に変えるcommandを案内しない。
    let command = format!(
        "sbxm add {} --worktrees {}",
        metadata.repository.clone_url(),
        provisioning.requested_worktrees
    );
    match provisioning.mode {
        CreationMode::Attached => Ok(command),
        CreationMode::Detached => match provisioning.start_ref.as_deref() {
            Some(branch) => Ok(format!("{command} --detach {branch}")),
            None => Err(Error::new(
                ErrorId::MetadataInvalidValue,
                msg!(
                    "error-metadata-invalid-value",
                    path = paths::display(&paths.metadata_file()),
                    field = "provisioning.start_ref",
                    detail = "detached mode requires an explicit start branch"
                ),
            )),
        },
    }
}
