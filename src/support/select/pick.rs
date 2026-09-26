use crate::diagnostics::{Msg, Result};

use super::{Candidate, ProjectPrompt, labels, unresolved};

/// 並べた候補から、promptで1件を選ぶ。候補が1件以上あることは呼び出し側が確かめておく。
///
/// 候補に対応しない選択はcancelではない。promptの契約違反として区別して報告する。
pub(super) fn pick(
    mut candidates: Vec<Candidate>,
    heading: &Msg,
    prompt: &mut dyn ProjectPrompt,
) -> Result<Candidate> {
    let index = prompt.select_one(heading, &labels(&candidates))?;
    if index >= candidates.len() {
        return Err(unresolved(index, candidates.len()));
    }
    Ok(candidates.remove(index))
}
