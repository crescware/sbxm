use crate::config::ConfigLocation;
use crate::diagnostics::{Msg, Result};
use crate::project::ProjectId;

use super::{Candidate, ProjectPrompt, candidates, find, labels, no_managed_projects, unresolved};

/// 引数、またはpromptで1件以上の案件を決める。
///
/// 引数は重複を除き、canonical ID昇順で返す。
pub fn many(
    location: &ConfigLocation,
    requested: &[ProjectId],
    heading: &Msg,
    prompt: &mut dyn ProjectPrompt,
) -> Result<Vec<Candidate>> {
    if !requested.is_empty() {
        let mut selected: Vec<Candidate> = Vec::new();
        for project in requested {
            let found = find(location, project)?;
            if !selected
                .iter()
                .any(|already| already.repository.canonical_id() == found.repository.canonical_id())
            {
                selected.push(found);
            }
        }
        selected.sort_by(|left, right| {
            left.repository
                .canonical_id()
                .as_str()
                .as_bytes()
                .cmp(right.repository.canonical_id().as_str().as_bytes())
        });
        return Ok(selected);
    }

    let candidates = candidates(location)?;
    if candidates.is_empty() {
        return Err(no_managed_projects());
    }
    let indexes = prompt.select_many(heading, &labels(&candidates))?;
    // 未選択の確定は受け付けない。操作せず終える場合はEscまたはCtrl-Cを使う。
    if indexes.is_empty() {
        return Err(unresolved(0, candidates.len()));
    }
    let mut selected = Vec::with_capacity(indexes.len());
    for index in indexes {
        let candidate = candidates
            .get(index)
            .ok_or_else(|| unresolved(index, candidates.len()))?;
        selected.push(candidate.clone());
    }
    Ok(selected)
}
