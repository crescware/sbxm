use crate::config::ConfigLocation;
use crate::diagnostics::{Msg, Result};
use crate::project::ProjectId;

use super::{Candidate, ProjectPrompt, candidates, find, no_local_projects, pick};

/// 引数、またはpromptで、hostにあるrepositoryを登録した案件を1件決める。
///
/// promptには`--local`で追加した案件だけを並べる。GitHubの案件を選ばせてから断らない。
/// `--local`の案件が1つも無ければ、選ばせる前に断る。引数で名指しされた案件は、種類を
/// 問わずそのまま返す。GitHubの案件を断る理由は、呼び出し側がその案件に即して示す。
pub fn one_local(
    location: &ConfigLocation,
    requested: Option<&ProjectId>,
    heading: &Msg,
    prompt: &mut dyn ProjectPrompt,
) -> Result<Candidate> {
    if let Some(project) = requested {
        return find(location, project);
    }
    let candidates: Vec<Candidate> = candidates(location)?
        .into_iter()
        .filter(|candidate| candidate.repository.host_path().is_some())
        .collect();
    if candidates.is_empty() {
        return Err(no_local_projects());
    }
    pick(candidates, heading, prompt)
}
