use crate::config::ConfigLocation;
use crate::diagnostics::{Msg, Result};
use crate::project::ProjectId;

use super::{Candidate, ProjectPrompt, candidates, find, labels, unresolved};

/// 引数、またはpromptで1件の案件を決める。
pub fn one(
    location: &ConfigLocation,
    requested: Option<&ProjectId>,
    heading: &Msg,
    prompt: &mut dyn ProjectPrompt,
) -> Result<Candidate> {
    if let Some(project) = requested {
        return find(location, project);
    }
    let mut candidates = candidates(location)?;
    let index = prompt.select_one(heading, &labels(&candidates))?;
    if index >= candidates.len() {
        return Err(unresolved(index, candidates.len()));
    }
    Ok(candidates.remove(index))
}
