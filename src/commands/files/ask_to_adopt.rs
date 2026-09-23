use std::path::Path;

use crate::diagnostics::{Error, ErrorId, Result};
use crate::i18n::{Catalog, Locale};
use crate::msg;
use crate::paths;
use crate::support::select::ProjectPrompt;

/// 差分を見た利用者に、Sandbox側の内容でhostの宣言fileを置き換えるかを訊く。
///
/// hostのfileを書き換える選択であるため、残す選択を先頭に置く。
pub fn ask_to_adopt(prompt: &mut dyn ProjectPrompt, source: &Path, locale: Locale) -> Result<bool> {
    let catalog = Catalog::new(locale);
    let label = |id: &str| match catalog.text(id) {
        Ok(text) => text,
        Err(failure) => failure.to_string(),
    };
    let choices = [
        label("prompt-files-pull-keep"),
        label("prompt-files-pull-adopt"),
    ];
    let heading = msg!("prompt-files-pull-heading", source = paths::display(source));
    let index = prompt.select_one(&heading, &choices)?;
    match index {
        0 => Ok(false),
        1 => Ok(true),
        // 候補に対応しない選択はcancelではない。promptの契約違反として区別して報告する。
        _ => Err(Error::new(
            ErrorId::SelectionUnresolved,
            msg!(
                "error-selection-unresolved",
                index = index,
                count = choices.len()
            ),
        )),
    }
}
