use crate::diagnostics::{Error, ErrorId, Result};
use crate::i18n::{Catalog, Locale};
use crate::msg;
use crate::support::select::ProjectPrompt;

/// 表示言語を選ばせる。
///
/// 見出しは選択前でも双方の利用者が読める固定の二言語表記とし、選択肢はその言語自身の
/// 自称表記で並べる。system localeから推測した言語を先頭へ置き、初期cursorに載せる。
pub fn ask_language(prompt: &mut dyn ProjectPrompt, guessed: Locale) -> Result<Locale> {
    let mut choices = vec![guessed];
    choices.extend(Locale::ALL.into_iter().filter(|locale| *locale != guessed));

    let items: Vec<String> = choices.iter().map(|locale| locale_name(*locale)).collect();
    let index = prompt.select_one(&msg!("prompt-language-heading"), &items)?;
    // 候補に対応しない選択はcancelではない。promptの契約違反として区別して報告する。
    choices.get(index).copied().ok_or_else(|| {
        Error::new(
            ErrorId::SelectionUnresolved,
            msg!(
                "error-selection-unresolved",
                index = index,
                count = choices.len()
            ),
        )
    })
}

/// その言語自身のresourceが持つ自称表記。
fn locale_name(locale: Locale) -> String {
    Catalog::new(locale)
        .text("locale-name")
        .unwrap_or_else(|failure| failure.to_string())
}

#[cfg(test)]
#[path = "ask_language_test.rs"]
mod ask_language_test;
