use std::path::Path;

use crate::commands::Context;
use crate::commands::add::Args;
use crate::commands::add::{ask_language, choose_git_identity, choose_language};
use crate::config::{self, GlobalConfig};
use crate::diagnostics::ErrorId;
use crate::i18n::Locale;
use crate::metadata::GitIdentity;

use crate::testing::outcome::{Checked, Refused, Required};

use crate::cli::Interactivity;
use crate::config::ConfigLocation;
use crate::testing::cli::{non_tty, tty};
use crate::testing::global_status::FakeHost;
use crate::testing::prompt::{ScriptedIdentityPrompt, ScriptedPrompt};

fn home() -> Checked<(tempfile::TempDir, ConfigLocation)> {
    let dir = tempfile::tempdir().required_because("temporary home")?;
    let location = ConfigLocation::from_home(dir.path().to_path_buf());
    Ok((dir, location))
}

/// `add`はSandboxのworkspaceを観測しない。読まれた場合に失敗する所在を名前で示す。
const UNOBSERVED_WORKSPACE_ROOT: &str = "/nonexistent/add-observes-no-workspace";

fn context(
    location: &ConfigLocation,
    lang: Option<Locale>,
    interactivity: Interactivity,
) -> Context<'_> {
    Context {
        location,
        workspace_root: Path::new(UNOBSERVED_WORKSPACE_ROOT),
        lang,
        interactivity,
    }
}

fn saved(location: &ConfigLocation) -> Checked<Option<Locale>> {
    Ok(config::load(location)
        .required_because("the configuration is readable")?
        .settings()
        .language)
}

#[test]
fn the_first_interactive_add_asks_once_and_saves_what_was_chosen() -> Checked {
    let (_dir, location) = home()?;
    let context = context(&location, None, tty());
    let mut prompt = ScriptedPrompt::choosing(0);

    let chosen = choose_language(&context, &GlobalConfig::default(), Locale::En, &mut prompt)
        .required_because("the language is chosen")?;

    assert_eq!(prompt.headings.borrow().len(), 1);
    assert_eq!(saved(&location)?, Some(chosen), "the choice is persisted");

    // 保存済みのlanguageがあれば、二度と訊かない。
    let config = GlobalConfig {
        language: Some(chosen),
        git_identity: None,
        files: Vec::new(),
    };
    let again = choose_language(&context, &config, Locale::En, &mut prompt)
        .required_because("a saved language needs no prompt")?;
    assert_eq!(again, chosen);
    assert_eq!(
        prompt.headings.borrow().len(),
        1,
        "the prompt is asked once"
    );
    Ok(())
}

#[test]
fn the_prompt_keeps_the_wording_both_kinds_of_user_can_read() -> Checked {
    let mut prompt = ScriptedPrompt::choosing(0);
    ask_language(&mut prompt, Locale::Ja).required_because("a language is chosen")?;

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
        ask_language(&mut prompt, Locale::En).required_because("a language is chosen")?,
        Locale::En
    );
    assert_eq!(
        prompt.asked.borrow()[0],
        vec!["English".to_string(), "日本語 / Japanese".to_string()]
    );
    Ok(())
}

#[test]
fn a_run_that_cannot_prompt_neither_asks_nor_saves() -> Checked {
    let (_dir, location) = home()?;
    let context = context(&location, None, non_tty());
    let mut prompt = ScriptedPrompt::choosing(0);

    let chosen = choose_language(&context, &GlobalConfig::default(), Locale::Ja, &mut prompt)
        .required_because("a non-interactive run keeps going")?;

    assert_eq!(chosen, Locale::Ja, "the run uses the resolved language");
    assert!(prompt.headings.borrow().is_empty());
    assert_eq!(saved(&location)?, None, "nothing is persisted");
    assert!(!location.dir().exists(), "no global state is created");
    Ok(())
}

#[test]
fn a_language_option_is_an_override_rather_than_a_choice_of_the_saved_one() -> Checked {
    let (_dir, location) = home()?;
    let context = context(&location, Some(Locale::Ja), tty());
    let mut prompt = ScriptedPrompt::choosing(0);

    // `--lang`はそのprocessだけのoverrideである。永続設定を選ぶpromptを省略しない。
    choose_language(&context, &GlobalConfig::default(), Locale::Ja, &mut prompt)
        .required_because("the language is chosen")?;
    assert_eq!(prompt.headings.borrow().len(), 1);

    // 保存済みlanguageがあれば、`--lang`で上書きしていてもpromptは出ない。
    let config = GlobalConfig {
        language: Some(Locale::En),
        git_identity: None,
        files: Vec::new(),
    };
    let mut prompt = ScriptedPrompt::choosing(0);
    assert_eq!(
        choose_language(&context, &config, Locale::Ja, &mut prompt)
            .required_because("no prompt")?,
        Locale::Ja,
        "the override decides this run"
    );
    assert!(prompt.headings.borrow().is_empty());
    Ok(())
}

#[test]
fn cancelling_the_prompt_changes_nothing_and_exits_with_130() -> Checked {
    let (_dir, location) = home()?;
    let context = context(&location, None, tty());
    let mut prompt = ScriptedPrompt::canceling();

    let error = choose_language(&context, &GlobalConfig::default(), Locale::En, &mut prompt)
        .refused_because("a cancelled prompt stops the run")?;

    assert_eq!(error.exit_code(), crate::diagnostics::ExitCode::Canceled);
    assert!(!location.dir().exists(), "nothing is created");
    Ok(())
}

// --- 名義の決め方 ---
//
// 決め方はTTYの有無、保存済みの既定の有無、宣言の有無で決まる。以下はその全組み合わせを
// 辿る。片方だけの宣言はCLI parseの段階で止まるため、`mod_test`が持つ。

fn identity() -> GitIdentity {
    crate::testing::metadata::git_identity()
}

fn add_args(git_identity: Option<GitIdentity>) -> Checked<Args> {
    Ok(Args {
        repository: crate::testing::project::ssh_repository("Example-Org/Example-Repo")?,
        worktrees: None,
        detach: None,
        git_identity,
    })
}

fn config_holding(git_identity: Option<GitIdentity>) -> GlobalConfig {
    GlobalConfig {
        language: Some(Locale::En),
        git_identity,
        files: Vec::new(),
    }
}

fn saved_identity(location: &ConfigLocation) -> Checked<Option<GitIdentity>> {
    Ok(config::load(location)
        .required_because("the configuration is readable")?
        .settings()
        .git_identity)
}

/// `git config --global`が名義を宣言しているhost。
fn host_declaring(name: &str, email: &str) -> FakeHost {
    FakeHost::new()
        .with_commands(&["git"])
        .responding(
            "git config --global --get-all user.name",
            &format!("{name}\n"),
        )
        .responding(
            "git config --global --get-all user.email",
            &format!("{email}\n"),
        )
}

/// 何も答えないhost。候補は空欄になる。
fn silent_host() -> FakeHost {
    FakeHost::new()
}

#[test]
fn the_first_interactive_add_asks_for_the_identity_and_saves_what_was_entered() -> Checked {
    let (_dir, location) = home()?;
    let context = context(&location, None, tty());
    let mut prompt = ScriptedIdentityPrompt::typing("Chosen User", "chosen@example.com");

    let chosen = choose_git_identity(
        &context,
        &config_holding(None),
        &add_args(None)?,
        &host_declaring("Host User", "host@example.com"),
        &mut prompt,
    )
    .required_because("the identity is chosen")?;

    assert_eq!(chosen.user_name, "Chosen User");
    assert_eq!(chosen.user_email, "chosen@example.com");
    assert_eq!(
        saved_identity(&location)?,
        Some(chosen),
        "the choice becomes the saved default"
    );
    Ok(())
}

#[test]
fn the_host_is_offered_as_a_candidate_rather_than_taken_as_the_value() -> Checked {
    let (_dir, location) = home()?;
    let context = context(&location, None, tty());
    // 何も打たなければ、置かれた候補がそのまま確定する。Enterだけを押した場合にあたる。
    let mut prompt = ScriptedIdentityPrompt::accepting_the_candidates();

    let chosen = choose_git_identity(
        &context,
        &config_holding(None),
        &add_args(None)?,
        &host_declaring("Host User", "host@example.com"),
        &mut prompt,
    )
    .required_because("the candidate is accepted")?;

    assert_eq!(
        prompt.candidate_for("prompt-git-user-name").as_deref(),
        Some("Host User"),
        "the host value is placed in the field"
    );
    assert_eq!(
        prompt.candidate_for("prompt-git-user-email").as_deref(),
        Some("host@example.com")
    );
    assert_eq!(chosen.user_name, "Host User");
    // 候補を経由しても、決めたのは利用者である。値は既定として保存される。
    assert_eq!(saved_identity(&location)?, Some(chosen));
    Ok(())
}

#[test]
fn a_host_that_declares_nothing_leaves_an_empty_field_rather_than_failing() -> Checked {
    let (_dir, location) = home()?;
    let context = context(&location, None, tty());
    let mut prompt = ScriptedIdentityPrompt::typing("Typed User", "typed@example.com");

    let chosen = choose_git_identity(
        &context,
        &config_holding(None),
        &add_args(None)?,
        &silent_host(),
        &mut prompt,
    )
    .required_because("an unobservable host does not stop the run")?;

    assert_eq!(
        prompt.candidate_for("prompt-git-user-name").as_deref(),
        Some(""),
        "the field starts empty"
    );
    assert_eq!(chosen.user_name, "Typed User");
    Ok(())
}

#[test]
fn a_saved_default_is_used_without_asking_again() -> Checked {
    let (_dir, location) = home()?;
    let mut prompt = ScriptedIdentityPrompt::typing("Never Asked", "never@example.com");

    for interactivity in [tty(), non_tty()] {
        let context = context(&location, None, interactivity);
        let chosen = choose_git_identity(
            &context,
            &config_holding(Some(identity())),
            &add_args(None)?,
            &host_declaring("Host User", "host@example.com"),
            &mut prompt,
        )
        .required_because("a saved default needs no prompt")?;

        // 既定が決まった後は、TTYの有無が結果を変えない。
        assert_eq!(chosen, identity());
    }
    assert!(!prompt.asked_anything(), "the prompt is never shown");
    Ok(())
}

#[test]
fn a_declared_identity_decides_the_run_without_asking_or_saving() -> Checked {
    let declared = GitIdentity {
        user_name: "Declared User".to_string(),
        user_email: "declared@example.com".to_string(),
    };

    for interactivity in [tty(), non_tty()] {
        for saved in [None, Some(identity())] {
            let (_dir, location) = home()?;
            let context = context(&location, None, interactivity);
            let mut prompt = ScriptedIdentityPrompt::typing("Never Asked", "never@example.com");

            let chosen = choose_git_identity(
                &context,
                &config_holding(saved.clone()),
                &add_args(Some(declared.clone()))?,
                &host_declaring("Host User", "host@example.com"),
                &mut prompt,
            )
            .required_because("a declared identity needs no prompt")?;

            assert_eq!(chosen, declared, "the declaration decides this run");
            assert!(!prompt.asked_anything());
            // `--lang`と同じく、そのprocessだけのoverrideであり既定を汚さない。
            assert_eq!(
                saved_identity(&location)?,
                None,
                "a declaration is never written to the configuration"
            );
        }
    }
    Ok(())
}

#[test]
fn a_run_that_can_neither_ask_nor_read_a_default_refuses_before_creating_anything() -> Checked {
    let (_dir, location) = home()?;
    let context = context(&location, None, non_tty());
    let mut prompt = ScriptedIdentityPrompt::typing("Never Asked", "never@example.com");

    let error = choose_git_identity(
        &context,
        &config_holding(None),
        &add_args(None)?,
        // hostが宣言していても、それを利用者の意図の代わりにはしない。
        &host_declaring("Host User", "host@example.com"),
        &mut prompt,
    )
    .refused_because("there is nothing that decides the identity")?;

    assert_eq!(error.first_id(), Some(ErrorId::GitIdentityUndecidable));
    assert!(!prompt.asked_anything());
    assert!(!location.dir().exists(), "no global state is created");
    Ok(())
}

#[test]
fn cancelling_the_identity_prompt_saves_nothing() -> Checked {
    let (_dir, location) = home()?;
    let context = context(&location, None, tty());
    let mut prompt = ScriptedIdentityPrompt::canceling();

    let error = choose_git_identity(
        &context,
        &config_holding(None),
        &add_args(None)?,
        &host_declaring("Host User", "host@example.com"),
        &mut prompt,
    )
    .refused_because("a cancelled prompt stops the run")?;

    assert_eq!(error.exit_code(), crate::diagnostics::ExitCode::Canceled);
    assert!(!location.dir().exists(), "nothing is created");
    Ok(())
}

#[test]
fn a_value_that_cannot_be_a_git_identity_is_refused_rather_than_saved() -> Checked {
    for (name, email) in [("", "user@example.com"), ("Example User", "   ")] {
        let (_dir, location) = home()?;
        let context = context(&location, None, tty());
        let mut prompt = ScriptedIdentityPrompt::typing(name, email);

        let error = choose_git_identity(
            &context,
            &config_holding(None),
            &add_args(None)?,
            &silent_host(),
            &mut prompt,
        )
        .refused_because("{name:?} {email:?} must be refused")?;

        assert_eq!(error.first_id(), Some(ErrorId::InvalidValue));
        assert_eq!(saved_identity(&location)?, None, "nothing is saved");
    }
    Ok(())
}
