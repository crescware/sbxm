use crate::boundary::host::HostEnvironment;
use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::metadata::ProjectMetadata;
use crate::msg;
use crate::project::SandboxLayout;

use crate::support::{repository, sandbox};

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
) -> Result<Vec<WorktreeRow>> {
    let provisioning = &metadata.provisioning;
    let git_dir = layout.bare_git_dir();
    let names = layout.worktree_names(provisioning.requested_worktrees);
    let created_from = provisioning
        .start_ref
        .as_deref()
        .map(crate::git::origin_ref)
        .unwrap_or_default();
    let mut rows = Vec::with_capacity(names.len());
    for name in names {
        let path = format!("{}/{name}", layout.bare_root());
        repository::adopt_worktree(host, sandbox, &git_dir, &path)?;
        let head = read_head(host, sandbox, &path)?;
        rows.push(WorktreeRow {
            path: name,
            created_from: created_from.clone(),
            head,
            mode: provisioning.mode,
        });
    }
    Ok(rows)
}

/// worktreeのHEADを読む。失敗、または空の応答は観測不能として拒否する。
fn read_head(host: &dyn HostEnvironment, sandbox_name: &str, path: &str) -> Result<String> {
    let outcome = sandbox::exec(
        host,
        sandbox_name,
        &["git", "-C", path, "rev-parse", "HEAD"],
    )?;
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
        .fact(Fact::path(path))
        .fact(Fact::reason(msg!("cause-head-unobservable"))),
    ))
}
