use crate::diagnostics::{Error, ErrorId, Result};
use crate::i18n::{Catalog, Locale};
use crate::msg;
use crate::support::select::ProjectPrompt;

/// 足した宣言を、登録済みの全案件へ今すぐ配置するかを訊く。
///
/// 既定は「今すぐ配置する」を先頭に置く。宣言を足した利用者の多くは、それを使うために
/// 足している。配置しない選択も同じ画面で選べる。
pub fn ask_to_apply(prompt: &mut dyn ProjectPrompt, count: usize, locale: Locale) -> Result<bool> {
    let catalog = Catalog::new(locale);
    let label = |id: &str| {
        catalog
            .text(id)
            .unwrap_or_else(|failure| failure.to_string())
    };
    let choices = [
        label("prompt-files-apply-now"),
        label("prompt-files-apply-later"),
    ];
    let index = prompt.select_one(&msg!("prompt-files-apply-heading", count = count), &choices)?;
    match index {
        0 => Ok(true),
        1 => Ok(false),
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
