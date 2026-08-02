use crate::design::policy::StreamPolicy;
use crate::diagnostics::{ErrorId, ExitCode};
use crate::i18n::{Catalog, Locale};
use crate::msg;

use crate::testing::outcome::{Checked, Refused, Required};

use super::PromptUi;

#[test]
fn a_prompt_without_candidates_is_unresolved_rather_than_cancelled() -> Checked {
    // 端末には触れない。並べるものが無いことは、打鍵を待つ前に決まる。
    let mut prompt = PromptUi::new(Locale::En, StreamPolicy::plain());
    let error = prompt
        .select_one(&msg!("select-open-heading"), &[])
        .refused_because("there is nothing to choose from")?;

    assert_eq!(error.first_id(), Some(ErrorId::SelectionUnresolved));
    // 何も選ばれなかった点は同じでも、利用者が取り消したのではない。
    assert_ne!(error.exit_code(), ExitCode::Canceled);

    // どの選択が、何件のうちのどれに対応しなかったかを報告が持つ。
    let diagnostic = error
        .diagnostics()
        .first()
        .required_because("the refusal is reported")?;
    let described = Catalog::new(Locale::En)
        .format(&diagnostic.description)
        .required_because("the report is readable")?;
    assert!(described.contains(" 0 candidates"), "{described}");
    Ok(())
}
