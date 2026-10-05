use crate::boundary::host::HostEnvironment;
use crate::design::{Fact, ProgressSink};
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::metadata::ProjectMetadata;
use crate::msg;
use crate::project::SandboxLayout;

use crate::support::{repository, worktree};

use super::WorktreeRow;

/// metadataが宣言するmanaged worktreeの現在の状態。
///
/// 各worktreeが、この案件の共有bare repositoryのworktreeであり続けていることを
/// `adopt_worktree`と同じ検査で確認する。branch・mode・HEADの値そのものは利用者の
/// 作業で変わり得るため一致条件にしないが、HEADを読めなかった場合は`None`へ丸めず
/// 拒否する。観測できなかった状態を、観測できた状態と同じ形で返さないためである。
pub(crate) fn observed_worktrees(
    host: &dyn HostEnvironment,
    sandbox: &str,
    layout: &SandboxLayout,
    metadata: &ProjectMetadata,
    names: &[String],
    progress: &mut dyn ProgressSink,
) -> Result<Vec<WorktreeRow>> {
    let provisioning = &metadata.provisioning;
    let git_dir = layout.bare_git_dir();
    let created_from = provisioning
        .start_ref
        .as_deref()
        .map(crate::git::origin_ref)
        .unwrap_or_default();
    let mut rows = Vec::with_capacity(names.len());
    for (index, name) in names.iter().enumerate() {
        let path = format!("{}/{name}", layout.bare_root());
        let mut inspection =
            worktree::Inspection::new(host, sandbox, &path, index + 1, names.len(), progress);
        repository::adopt_worktree(&mut inspection, &git_dir)?;
        let head = read_head(&mut inspection)?;
        rows.push(WorktreeRow {
            path: name.clone(),
            created_from: created_from.clone(),
            head,
            mode: provisioning.mode,
        });
    }
    Ok(rows)
}

/// worktreeのHEADを読む。失敗、または空の応答は観測不能として拒否する。
fn read_head(inspection: &mut worktree::Inspection<'_>) -> Result<String> {
    let outcome = inspection.head_outcome()?;
    let observed = outcome.stdout_text();
    let trimmed = observed.trim();
    if outcome.success() && !trimmed.is_empty() {
        return Ok(trimmed.to_string());
    }
    Err(Error::single(
        Diagnostic::new(
            ErrorId::SandboxRepositoryUnusable,
            msg!("error-sandbox-repository-unusable"),
        )
        .fact(Fact::path(inspection.path()))
        .fact(Fact::reason(msg!("cause-head-unobservable"))),
    ))
}
