use super::*;
use crate::cli::Interactivity;
use crate::config::ConfigLocation;
use crate::testing::cli::{non_tty, tty};
use crate::testing::prompt::ScriptedPrompt;

fn home() -> (tempfile::TempDir, ConfigLocation) {
    let dir = tempfile::tempdir().expect("temporary home");
    let location = ConfigLocation::from_home(dir.path().to_path_buf());
    (dir, location)
}

fn context<'a>(
    location: &'a ConfigLocation,
    lang: Option<Locale>,
    interactivity: Interactivity,
) -> Context<'a> {
    Context {
        location,
        lang,
        interactivity,
    }
}

fn saved(location: &ConfigLocation) -> Option<Locale> {
    config::load(location)
        .expect("the configuration is readable")
        .settings()
        .language
}

#[test]
fn the_first_interactive_add_asks_once_and_saves_what_was_chosen() {
    let (_dir, location) = home();
    let context = context(&location, None, tty());
    let mut prompt = ScriptedPrompt::choosing(0);

    let chosen = choose_language(&context, &GlobalConfig::default(), Locale::En, &mut prompt)
        .expect("the language is chosen");

    assert_eq!(prompt.headings.borrow().len(), 1);
    assert_eq!(saved(&location), Some(chosen), "the choice is persisted");

    // 保存済みのlanguageがあれば、二度と訊かない。
    let config = GlobalConfig {
        language: Some(chosen),
        files: Vec::new(),
    };
    let again = choose_language(&context, &config, Locale::En, &mut prompt)
        .expect("a saved language needs no prompt");
    assert_eq!(again, chosen);
    assert_eq!(
        prompt.headings.borrow().len(),
        1,
        "the prompt is asked once"
    );
}

#[test]
fn the_prompt_keeps_the_wording_both_kinds_of_user_can_read() {
    let mut prompt = ScriptedPrompt::choosing(0);
    ask_language(&mut prompt, Locale::Ja).expect("a language is chosen");

    assert_eq!(
        prompt.headings.borrow().as_slice(),
        ["prompt-language-heading"],
        "the heading is fixed and bilingual"
    );
    assert_eq!(
        prompt.asked.borrow()[0],
        vec!["日本語 / Japanese".to_string(), "English".to_string()],
        "each choice names itself"
    );

    // system localeから推測した言語を初期cursor位置、つまり先頭へ置く。
    let mut prompt = ScriptedPrompt::choosing(0);
    assert_eq!(
        ask_language(&mut prompt, Locale::En).expect("a language is chosen"),
        Locale::En
    );
    assert_eq!(
        prompt.asked.borrow()[0],
        vec!["English".to_string(), "日本語 / Japanese".to_string()]
    );
}

#[test]
fn a_run_that_cannot_prompt_neither_asks_nor_saves() {
    let (_dir, location) = home();
    let context = context(&location, None, non_tty());
    let mut prompt = ScriptedPrompt::choosing(0);

    let chosen = choose_language(&context, &GlobalConfig::default(), Locale::Ja, &mut prompt)
        .expect("a non-interactive run keeps going");

    assert_eq!(chosen, Locale::Ja, "the run uses the resolved language");
    assert!(prompt.headings.borrow().is_empty());
    assert_eq!(saved(&location), None, "nothing is persisted");
    assert!(!location.dir().exists(), "no global state is created");
}

#[test]
fn a_language_option_is_an_override_rather_than_a_choice_of_the_saved_one() {
    let (_dir, location) = home();
    let context = context(&location, Some(Locale::Ja), tty());
    let mut prompt = ScriptedPrompt::choosing(0);

    // `--lang`はそのprocessだけのoverrideである。永続設定を選ぶpromptを省略しない。
    choose_language(&context, &GlobalConfig::default(), Locale::Ja, &mut prompt)
        .expect("the language is chosen");
    assert_eq!(prompt.headings.borrow().len(), 1);

    // 保存済みlanguageがあれば、`--lang`で上書きしていてもpromptは出ない。
    let config = GlobalConfig {
        language: Some(Locale::En),
        files: Vec::new(),
    };
    let mut prompt = ScriptedPrompt::choosing(0);
    assert_eq!(
        choose_language(&context, &config, Locale::Ja, &mut prompt).expect("no prompt"),
        Locale::Ja,
        "the override decides this run"
    );
    assert!(prompt.headings.borrow().is_empty());
}

#[test]
fn cancelling_the_prompt_changes_nothing_and_exits_with_130() {
    let (_dir, location) = home();
    let context = context(&location, None, tty());
    let mut prompt = ScriptedPrompt::canceling();

    let error = choose_language(&context, &GlobalConfig::default(), Locale::En, &mut prompt)
        .expect_err("a cancelled prompt stops the run");

    assert_eq!(error.exit_code(), crate::error::ExitCode::Canceled);
    assert!(!location.dir().exists(), "nothing is created");
}
