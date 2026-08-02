use crate::diagnostics::{ErrorId, ExitCode};

use crate::testing::outcome::{Checked, Refused, Required};

use super::*;
use crate::testing::prompt::ScriptedPrompt;

/// 各localeが自分のresourceで名乗る表記。
///
/// 実装を呼び直すと同じ写像を二度書くだけになるため、期待値はtest側の表として持つ。
/// 網羅matchにしておくと、localeを足したときにこの表も更新を強制される。
fn self_designation(locale: Locale) -> &'static str {
    match locale {
        Locale::En => "English",
        Locale::Ja => "日本語 / Japanese",
    }
}

/// 推測したlocaleを先頭に、残りの組み込みlocaleが元の順で続く並び。
fn expected_candidates(guessed: Locale) -> Vec<String> {
    std::iter::once(guessed)
        .chain(Locale::ALL.into_iter().filter(|locale| *locale != guessed))
        .map(|locale| self_designation(locale).to_string())
        .collect()
}

#[test]
fn the_guessed_language_is_offered_first_and_every_other_shipped_locale_follows_it_once() -> Checked
{
    for guessed in Locale::ALL {
        let mut prompt = ScriptedPrompt::choosing(0);
        let chosen = ask_language(&mut prompt, guessed)
            .required_because("the first candidate is offered and chosen")?;
        assert_eq!(
            chosen, guessed,
            "the guessed language sits on the initial cursor"
        );

        let asked = prompt.asked.borrow();
        let candidates = asked.first().required_because("the user is asked once")?;
        assert_eq!(
            *candidates,
            expected_candidates(guessed),
            "the guess leads and the remaining locales follow without repeating it"
        );
        assert_eq!(
            candidates.len(),
            Locale::ALL.len(),
            "every shipped locale is offered exactly once"
        );
        assert_eq!(
            prompt.headings.borrow().as_slice(),
            ["prompt-language-heading"]
        );
    }
    Ok(())
}

/// 候補は表示中の言語ではなく、その言語自身の綴りで並ぶ。
#[test]
fn every_language_is_offered_in_its_own_words_whichever_one_was_guessed() -> Checked {
    for guessed in Locale::ALL {
        let mut prompt = ScriptedPrompt::choosing(0);
        ask_language(&mut prompt, guessed).required_because("the prompt answers")?;
        let asked = prompt.asked.borrow();
        let candidates = asked.first().required_because("the user is asked once")?;
        assert!(
            candidates.contains(&"日本語 / Japanese".to_string()),
            "Japanese names itself in Japanese even when {} is guessed",
            guessed.as_str()
        );
        assert!(
            candidates.contains(&"English".to_string()),
            "English names itself in English even when {} is guessed",
            guessed.as_str()
        );
    }
    Ok(())
}

/// 先頭以外を選べば、その位置が指す言語が確定する。
#[test]
fn a_language_chosen_after_the_first_is_the_one_that_position_names() -> Checked {
    for guessed in Locale::ALL {
        let last = Locale::ALL.len() - 1;
        let mut prompt = ScriptedPrompt::choosing(last);
        let chosen =
            ask_language(&mut prompt, guessed).required_because("the last candidate is chosen")?;
        assert_eq!(
            self_designation(chosen),
            expected_candidates(guessed)[last],
            "the answer resolves to the candidate at that position"
        );
        assert_ne!(
            chosen, guessed,
            "choosing past the guess changes the display language"
        );
    }
    Ok(())
}

#[test]
fn a_selection_that_names_no_candidate_is_refused_with_the_index_and_the_candidate_count() -> Checked
{
    // 端のひとつ外と、明らかに離れた値。どちらも受け取った値のまま報告されることを見る。
    for index in [Locale::ALL.len(), 9] {
        let mut prompt = ScriptedPrompt::choosing(index);
        let error = ask_language(&mut prompt, Locale::Ja)
            .refused_because("an answer outside the candidates is not a selection")?;
        assert_eq!(error.first_id(), Some(ErrorId::SelectionUnresolved));
        // promptの契約違反であり、利用者が降りたわけではない。
        assert_ne!(error.exit_code(), ExitCode::Canceled);

        let description = &error.diagnostics()[0].description;
        assert_eq!(description.id, "error-selection-unresolved");
        let argument = |key: &str| {
            description
                .args
                .iter()
                .find(|(name, _)| *name == key)
                .map(|(_, value)| value.clone())
        };
        assert_eq!(
            argument("index"),
            Some(index.to_string()),
            "the index the prompt answered with is carried into the diagnostic"
        );
        let offered = prompt.asked.borrow()[0].len();
        assert_eq!(
            argument("count"),
            Some(offered.to_string()),
            "the number of candidates actually offered is carried into the diagnostic"
        );
    }
    Ok(())
}
