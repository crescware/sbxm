use crate::config::ConfigLocation;
use crate::diagnostics::{Msg, Result};

use super::{Candidate, ProjectPrompt, candidates, labels, no_managed_projects, unresolved};

/// 案件とworktree indexを1画面で選ぶ。
///
/// promptにはregistryの表示情報だけを渡す。metadataの最大値を読むとprompt表示が遅れる
/// ため、indexは呼び出し側が渡す楽観的な上限で受け付け、確定後にlock済みmetadataで
/// clampする。
pub fn open(
    location: &ConfigLocation,
    heading: &Msg,
    prompt: &mut dyn ProjectPrompt,
    maximum_index: u32,
) -> Result<(Candidate, u32)> {
    let mut candidates = candidates(location)?;
    if candidates.is_empty() {
        return Err(no_managed_projects());
    }
    let labels = labels(&candidates);
    let (project, index) = prompt.select_open(heading, &labels, maximum_index)?;
    if project >= candidates.len() {
        return Err(unresolved(project, candidates.len()));
    }
    Ok((candidates.remove(project), index))
}
