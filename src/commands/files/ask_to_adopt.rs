use std::path::Path;

use crate::diagnostics::Result;
use crate::i18n::Locale;
use crate::msg;
use crate::paths;
use crate::support::select::{self, ProjectPrompt};

/// 差分を見た利用者に、Sandbox側の内容でhostの宣言fileを置き換えるかを訊く。
///
/// hostのfileを書き換える選択であるため、残す選択を先頭に置く。
pub fn ask_to_adopt(prompt: &mut dyn ProjectPrompt, source: &Path, locale: Locale) -> Result<bool> {
    let keep = select::chooses_first(
        prompt,
        &msg!("prompt-files-pull-heading", source = paths::display(source)),
        "prompt-files-pull-keep",
        "prompt-files-pull-adopt",
        locale,
    )?;
    Ok(!keep)
}
