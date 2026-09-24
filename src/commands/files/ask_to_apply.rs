use crate::diagnostics::Result;
use crate::i18n::Locale;
use crate::msg;
use crate::support::select::{self, ProjectPrompt};

/// 足した宣言を、登録済みの全案件へ今すぐ配置するかを訊く。
///
/// 既定は「今すぐ配置する」を先頭に置く。宣言を足した利用者の多くは、それを使うために
/// 足している。配置しない選択も同じ画面で選べる。
pub fn ask_to_apply(prompt: &mut dyn ProjectPrompt, count: usize, locale: Locale) -> Result<bool> {
    select::chooses_first(
        prompt,
        &msg!("prompt-files-apply-heading", count = count),
        "prompt-files-apply-now",
        "prompt-files-apply-later",
        locale,
    )
}
