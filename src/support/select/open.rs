use crate::config::ConfigLocation;
use crate::diagnostics::{Msg, Result};
use crate::metadata::last_worktree_index;

use super::{Candidate, ProjectPrompt, candidates, labels, no_managed_projects, unresolved};

/// 案件とworktree indexを1画面で選ぶ。
///
/// promptを開く前に、全案件のmetadataをlockを取らずに読み、案件ごとの範囲を渡す。
/// 読むのは範囲を示すためだけであり、判定はlock後に読み直したmetadataで行う。
/// metadataを読めない案件も候補から外さない。1案件の破損でprompt全体を失わせず、
/// 読めない理由はその案件を選んだときにlock後の読み直しが述べる。
pub fn open(
    location: &ConfigLocation,
    heading: &Msg,
    prompt: &mut dyn ProjectPrompt,
) -> Result<(Candidate, u32)> {
    let mut candidates = candidates(location)?;
    if candidates.is_empty() {
        return Err(no_managed_projects());
    }
    let labels = labels(&candidates);
    let maximums: Vec<Option<u32>> = candidates.iter().map(maximum_index).collect();
    let (project, index) = prompt.select_open(heading, &labels, &maximums)?;
    if project >= candidates.len() {
        return Err(unresolved(project, candidates.len()));
    }
    Ok((candidates.remove(project), index))
}

/// 案件が宣言するmanaged worktreeの最後のindex。metadataを読めなければ`None`。
fn maximum_index(candidate: &Candidate) -> Option<u32> {
    candidate
        .reload()
        .ok()
        .map(|metadata| last_worktree_index(metadata.provisioning.requested_worktrees))
}
