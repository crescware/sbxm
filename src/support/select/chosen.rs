use crate::config::ConfigLocation;
use crate::diagnostics::{Msg, Result};
use crate::project::ProjectId;

use super::{ProjectPrompt, one};

/// 引数、またはpromptで1件の案件を決め、その識別子を返す。
///
/// 同じ案件へ操作をやり直す呼び出し側が、やり直すたびに選び直させないために使う。
/// lockは取らない。対象の状態はlockを取った側が読み直す。
pub fn chosen(
    location: &ConfigLocation,
    requested: Option<&ProjectId>,
    heading: &Msg,
    prompt: &mut dyn ProjectPrompt,
) -> Result<ProjectId> {
    ProjectId::parse(&one(location, requested, heading, prompt)?.display_id())
}
