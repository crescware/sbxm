use crate::diagnostics::{Msg, Result};
use crate::i18n::{Catalog, Locale};

use super::{ProjectPrompt, unresolved};

/// 2つの選択肢を示し、1つ目が選ばれたかを返す。選択肢はmessage IDで渡す。
///
/// 候補に対応しない選択はcancelではない。promptの契約違反として区別して報告する。
pub fn chooses_first(
    prompt: &mut dyn ProjectPrompt,
    heading: &Msg,
    first: &str,
    second: &str,
    locale: Locale,
) -> Result<bool> {
    let catalog = Catalog::new(locale);
    let choices = [first, second].map(|id| match catalog.text(id) {
        Ok(text) => text,
        Err(failure) => failure.to_string(),
    });
    match prompt.select_one(heading, &choices)? {
        0 => Ok(true),
        1 => Ok(false),
        index => Err(unresolved(index, choices.len())),
    }
}
