use super::*;
use crate::error::ExitCode;
use crate::msg;
use crate::testing::project::{fixture, project_id};
use crate::testing::prompt::ScriptedPrompt;

#[test]
fn a_named_project_is_used_without_asking() {
    let fixture = fixture();
    fixture.register("example-org/example-repo");
    fixture.register("other/other-repo");

    let mut prompt = ScriptedPrompt::choosing(1);
    let chosen = one(
        &fixture.location,
        Some(&project_id("Example-Org/Example-Repo")),
        msg!("select-open-heading"),
        &mut prompt,
    )
    .expect("the named project is found");
    assert_eq!(chosen.display_id(), "example-org/example-repo");
    assert!(
        prompt.asked.borrow().is_empty(),
        "a named target never prompts"
    );
}

#[test]
fn a_named_project_is_read_without_discovering_the_others() {
    let fixture = fixture();
    fixture.register("example-org/example-repo");

    // 無関係な案件のmetadataが壊れていても、完全指定された対象は読める。
    let broken = fixture.parent.as_path().join("broken/broken.project/.sbxm");
    std::fs::create_dir_all(&broken).unwrap();
    std::fs::write(broken.join("project.yaml"), "version: 2\n").unwrap();

    let chosen = one(
        &fixture.location,
        Some(&project_id("example-org/example-repo")),
        msg!("select-open-heading"),
        &mut ScriptedPrompt::choosing(0),
    )
    .expect("an unrelated project does not decide this one");
    assert_eq!(chosen.display_id(), "example-org/example-repo");
}

#[test]
fn an_omitted_target_is_chosen_from_the_managed_projects() {
    let fixture = fixture();
    fixture.register("example-org/example-repo");
    fixture.register("other/other-repo");

    let mut prompt = ScriptedPrompt::choosing(1);
    let chosen = one(
        &fixture.location,
        None,
        msg!("select-open-heading"),
        &mut prompt,
    )
    .expect("select");
    assert_eq!(chosen.display_id(), "other/other-repo");
    assert_eq!(
        prompt.asked.borrow()[0],
        vec![
            "example-org/example-repo".to_string(),
            "other/other-repo".to_string()
        ],
        "candidates are listed in canonical order"
    );
}

#[test]
fn cancelling_the_prompt_changes_nothing() {
    let fixture = fixture();
    fixture.register("example-org/example-repo");

    let error = one(
        &fixture.location,
        None,
        msg!("select-open-heading"),
        &mut ScriptedPrompt::canceling(),
    )
    .expect_err("a cancelled prompt is not a selection");
    assert_eq!(error.exit_code(), ExitCode::Canceled);
}

#[test]
fn no_managed_project_is_an_error_rather_than_an_empty_prompt() {
    let fixture = fixture();

    let mut prompt = ScriptedPrompt::choosing(0);
    let error = one(
        &fixture.location,
        None,
        msg!("select-open-heading"),
        &mut prompt,
    )
    .expect_err("there is nothing to choose from");
    assert_eq!(error.first_id(), Some(ErrorId::NoManagedProjects));
    assert!(prompt.asked.borrow().is_empty(), "no empty prompt is shown");

    let error = many(
        &fixture.location,
        &[],
        msg!("select-stop-heading"),
        &mut prompt,
    )
    .expect_err("the same holds for many");
    assert_eq!(error.first_id(), Some(ErrorId::NoManagedProjects));
}

#[test]
fn a_selection_that_matches_no_candidate_is_not_a_cancel() {
    let fixture = fixture();
    fixture.register("example-org/example-repo");

    let error = one(
        &fixture.location,
        None,
        msg!("select-open-heading"),
        &mut ScriptedPrompt::choosing(7),
    )
    .expect_err("an answer outside the candidates is not a selection");
    assert_eq!(error.first_id(), Some(ErrorId::SelectionUnresolved));

    // 未選択の確定も、対象が決まらなかったこととして扱う。
    let error = many(
        &fixture.location,
        &[],
        msg!("select-stop-heading"),
        &mut ScriptedPrompt::choosing_many(&[]),
    )
    .expect_err("confirming nothing selects nothing");
    assert_eq!(error.first_id(), Some(ErrorId::SelectionUnresolved));
}

#[test]
fn several_named_projects_are_deduplicated_and_ordered() {
    let fixture = fixture();
    fixture.register("zeta/zulu");
    fixture.register("alpha/alfa");

    let mut prompt = ScriptedPrompt::choosing_many(&[0]);
    let selected = many(
        &fixture.location,
        &[
            project_id("Zeta/Zulu"),
            project_id("alpha/alfa"),
            project_id("zeta/zulu"),
        ],
        msg!("select-stop-heading"),
        &mut prompt,
    )
    .expect("select");
    assert_eq!(
        selected
            .iter()
            .map(|project| project.display_id())
            .collect::<Vec<_>>(),
        vec!["alpha/alfa".to_string(), "zeta/zulu".to_string()]
    );
    assert!(prompt.asked.borrow().is_empty());
}

#[test]
fn a_project_that_is_not_managed_is_named_in_the_diagnostic() {
    let fixture = fixture();
    fixture.register("example-org/example-repo");

    let mut prompt = ScriptedPrompt::choosing(0);
    let error = one(
        &fixture.location,
        Some(&project_id("other/other-repo")),
        msg!("select-open-heading"),
        &mut prompt,
    )
    .expect_err("an unmanaged project cannot be the target");
    assert_eq!(error.first_id(), Some(ErrorId::ProjectNotManaged));
}

#[test]
fn a_project_that_disappears_before_the_lock_is_named_as_its_metadata_spelled_it() {
    let fixture = fixture();
    let project = fixture.register("Example-Org/Example-Repo");

    let candidate = one(
        &fixture.location,
        Some(&project_id("example-org/example-repo")),
        msg!("select-open-heading"),
        &mut ScriptedPrompt::choosing(0),
    )
    .expect("the project is managed when it is selected");

    // 選択とlock後の読み直しのあいだに、並行するdestroyがmetadataを消すことがある。
    // registry entryは残るため、案件は未登録ではなく登録途中として報告される。
    std::fs::remove_file(project.paths.metadata_file()).expect("remove the metadata");

    let error = candidate
        .lock()
        .expect_err("a project without metadata cannot be worked on");
    assert_eq!(error.first_id(), Some(ErrorId::ProjectIncomplete));
    let diagnostic = &error.diagnostics()[0];
    assert_eq!(
        diagnostic
            .description
            .args
            .iter()
            .find(|(key, _)| *key == "project")
            .map(|(_, value)| value.as_str()),
        Some("Example-Org/Example-Repo"),
        "the registered spelling is reported, not the one the argument used"
    );
    let remediation = diagnostic
        .remediation
        .as_ref()
        .expect("the user is told how to continue the registration");
    // 実行を求めるcommandは説明文へ埋め込まず、独立した一行として持つ。
    assert_eq!(
        remediation
            .commands
            .iter()
            .map(|command| command.as_str())
            .collect::<Vec<_>>(),
        vec!["sbxm add git@github.com:Example-Org/Example-Repo.git"]
    );
}

#[test]
fn an_interrupted_prompt_is_a_cancel_and_any_other_read_failure_is_reported() {
    let canceled =
        crate::ui::prompt::unreadable(std::io::Error::from(std::io::ErrorKind::Interrupted));
    assert_eq!(canceled.exit_code(), ExitCode::Canceled);

    let unreadable =
        crate::ui::prompt::unreadable(std::io::Error::from(std::io::ErrorKind::BrokenPipe));
    assert_eq!(unreadable.first_id(), Some(ErrorId::PromptUnreadable));
    assert_ne!(unreadable.exit_code(), ExitCode::Canceled);
}
