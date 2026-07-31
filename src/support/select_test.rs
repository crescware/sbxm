use crate::testing::outcome::{Checked, Refused, Required};

use super::*;
use crate::error::ExitCode;
use crate::msg;
use crate::testing::project::{fixture, project_id};
use crate::testing::prompt::ScriptedPrompt;

#[test]
fn a_named_project_is_used_without_asking() -> Checked {
    let fixture = fixture()?;
    fixture.register("example-org/example-repo")?;
    fixture.register("other/other-repo")?;

    let mut prompt = ScriptedPrompt::choosing(1);
    let chosen = one(
        &fixture.location,
        Some(&project_id("Example-Org/Example-Repo")?),
        &msg!("select-open-heading"),
        &mut prompt,
    )
    .required_because("the named project is found")?;
    assert_eq!(chosen.display_id(), "example-org/example-repo");
    assert!(
        prompt.asked.borrow().is_empty(),
        "a named target never prompts"
    );
    Ok(())
}

#[test]
fn a_named_project_is_read_without_discovering_the_others() -> Checked {
    let fixture = fixture()?;
    fixture.register("example-org/example-repo")?;

    // 無関係な案件のmetadataが壊れていても、完全指定された対象は読める。
    let broken = fixture.parent.as_path().join("broken/broken.project/.sbxm");
    std::fs::create_dir_all(&broken).required()?;
    std::fs::write(broken.join("project.yaml"), "version: 2\n").required()?;

    let chosen = one(
        &fixture.location,
        Some(&project_id("example-org/example-repo")?),
        &msg!("select-open-heading"),
        &mut ScriptedPrompt::choosing(0),
    )
    .required_because("an unrelated project does not decide this one")?;
    assert_eq!(chosen.display_id(), "example-org/example-repo");
    Ok(())
}

#[test]
fn an_omitted_target_is_chosen_from_the_managed_projects() -> Checked {
    let fixture = fixture()?;
    fixture.register("example-org/example-repo")?;
    fixture.register("other/other-repo")?;

    let mut prompt = ScriptedPrompt::choosing(1);
    let chosen = one(
        &fixture.location,
        None,
        &msg!("select-open-heading"),
        &mut prompt,
    )
    .required_because("select")?;
    assert_eq!(chosen.display_id(), "other/other-repo");
    assert_eq!(
        prompt.asked.borrow()[0],
        vec![
            "example-org/example-repo".to_string(),
            "other/other-repo".to_string()
        ],
        "candidates are listed in canonical order"
    );
    Ok(())
}

#[test]
fn cancelling_the_prompt_changes_nothing() -> Checked {
    let fixture = fixture()?;
    fixture.register("example-org/example-repo")?;

    let error = one(
        &fixture.location,
        None,
        &msg!("select-open-heading"),
        &mut ScriptedPrompt::canceling(),
    )
    .refused_because("a cancelled prompt is not a selection")?;
    assert_eq!(error.exit_code(), ExitCode::Canceled);
    Ok(())
}

#[test]
fn no_managed_project_is_an_error_rather_than_an_empty_prompt() -> Checked {
    let fixture = fixture()?;

    let mut prompt = ScriptedPrompt::choosing(0);
    let error = one(
        &fixture.location,
        None,
        &msg!("select-open-heading"),
        &mut prompt,
    )
    .refused_because("there is nothing to choose from")?;
    assert_eq!(error.first_id(), Some(ErrorId::NoManagedProjects));
    assert!(prompt.asked.borrow().is_empty(), "no empty prompt is shown");

    let error = many(
        &fixture.location,
        &[],
        &msg!("select-stop-heading"),
        &mut prompt,
    )
    .refused_because("the same holds for many")?;
    assert_eq!(error.first_id(), Some(ErrorId::NoManagedProjects));
    Ok(())
}

#[test]
fn a_selection_that_matches_no_candidate_is_not_a_cancel() -> Checked {
    let fixture = fixture()?;
    fixture.register("example-org/example-repo")?;

    let error = one(
        &fixture.location,
        None,
        &msg!("select-open-heading"),
        &mut ScriptedPrompt::choosing(7),
    )
    .refused_because("an answer outside the candidates is not a selection")?;
    assert_eq!(error.first_id(), Some(ErrorId::SelectionUnresolved));

    // 未選択の確定も、対象が決まらなかったこととして扱う。
    let error = many(
        &fixture.location,
        &[],
        &msg!("select-stop-heading"),
        &mut ScriptedPrompt::choosing_many(&[]),
    )
    .refused_because("confirming nothing selects nothing")?;
    assert_eq!(error.first_id(), Some(ErrorId::SelectionUnresolved));
    Ok(())
}

#[test]
fn several_named_projects_are_deduplicated_and_ordered() -> Checked {
    let fixture = fixture()?;
    fixture.register("zeta/zulu")?;
    fixture.register("alpha/alfa")?;

    let mut prompt = ScriptedPrompt::choosing_many(&[0]);
    let selected = many(
        &fixture.location,
        &[
            project_id("Zeta/Zulu")?,
            project_id("alpha/alfa")?,
            project_id("zeta/zulu")?,
        ],
        &msg!("select-stop-heading"),
        &mut prompt,
    )
    .required_because("select")?;
    assert_eq!(
        selected
            .iter()
            .map(super::Candidate::display_id)
            .collect::<Vec<_>>(),
        vec!["alpha/alfa".to_string(), "zeta/zulu".to_string()]
    );
    assert!(prompt.asked.borrow().is_empty());
    Ok(())
}

#[test]
fn a_project_that_is_not_managed_is_named_in_the_diagnostic() -> Checked {
    let fixture = fixture()?;
    fixture.register("example-org/example-repo")?;

    let mut prompt = ScriptedPrompt::choosing(0);
    let error = one(
        &fixture.location,
        Some(&project_id("other/other-repo")?),
        &msg!("select-open-heading"),
        &mut prompt,
    )
    .refused_because("an unmanaged project cannot be the target")?;
    assert_eq!(error.first_id(), Some(ErrorId::ProjectNotManaged));
    Ok(())
}

#[test]
fn a_project_that_disappears_before_the_lock_is_named_as_its_metadata_spelled_it() -> Checked {
    let fixture = fixture()?;
    let project = fixture.register("Example-Org/Example-Repo")?;

    let candidate = one(
        &fixture.location,
        Some(&project_id("example-org/example-repo")?),
        &msg!("select-open-heading"),
        &mut ScriptedPrompt::choosing(0),
    )
    .required_because("the project is managed when it is selected")?;

    // 選択とlock後の読み直しのあいだに、並行するdestroyがmetadataを消すことがある。
    // registry entryは残るため、案件は未登録ではなく登録途中として報告される。
    std::fs::remove_file(project.paths.metadata_file()).required_because("remove the metadata")?;

    let error = candidate
        .lock()
        .refused_because("a project without metadata cannot be worked on")?;
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
        .required_because("the user is told how to continue the registration")?;
    // 実行を求めるcommandは説明文へ埋め込まず、独立した一行として持つ。
    assert_eq!(
        remediation
            .commands
            .iter()
            .map(crate::ui::text::CommandLine::as_str)
            .collect::<Vec<_>>(),
        vec!["sbxm add git@github.com:Example-Org/Example-Repo.git"]
    );
    Ok(())
}

#[test]
fn an_interrupted_prompt_is_a_cancel_and_any_other_read_failure_is_reported() {
    let canceled =
        crate::ui::prompt::unreadable(&std::io::Error::from(std::io::ErrorKind::Interrupted));
    assert_eq!(canceled.exit_code(), ExitCode::Canceled);

    let unreadable =
        crate::ui::prompt::unreadable(&std::io::Error::from(std::io::ErrorKind::BrokenPipe));
    assert_eq!(unreadable.first_id(), Some(ErrorId::PromptUnreadable));
    assert_ne!(unreadable.exit_code(), ExitCode::Canceled);
}

#[test]
fn a_registry_path_is_observed_before_it_is_read_or_written() -> Checked {
    let fixture = fixture()?;
    let project = fixture.register("example-org/example-repo");
    let root = project?.paths.root().to_path_buf();

    // 保存済みのabsolute pathでも、そこにdirectoryがあることを確かめてから使う。
    std::fs::remove_dir_all(&root).required_because("undo the project root")?;
    std::fs::write(&root, b"not a directory").required_because("put something else there")?;

    let candidate = one(
        &fixture.location,
        Some(&project_id("example-org/example-repo")?),
        &msg!("select-open-heading"),
        &mut ScriptedPrompt::choosing(0),
    )
    .required_because("the entry still names the project")?;
    let error = candidate
        .lock()
        .refused_because("a path that is not a directory is never worked on")?;
    assert_eq!(error.first_id(), Some(ErrorId::ProjectPathUnexpectedType));

    // symlinkも追跡しない。指す先は案件directoryの外にあり得る。
    std::fs::remove_file(&root).required()?;
    let elsewhere = fixture.parent.as_path().join("elsewhere");
    std::fs::create_dir_all(&elsewhere).required()?;
    std::os::unix::fs::symlink(&elsewhere, &root).required()?;

    let error =
        crate::support::select::find(&fixture.location, &project_id("example-org/example-repo")?)
            .required_because("the entry still names the project")?
            .lock()
            .refused_because("a symlinked project root is refused")?;
    assert_eq!(error.first_id(), Some(ErrorId::ProjectPathSymlink));
    Ok(())
}
