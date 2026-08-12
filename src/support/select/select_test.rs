use crate::diagnostics::ErrorId;

use crate::testing::outcome::{Checked, Refused, Required};

use super::*;
use crate::diagnostics::ExitCode;
use crate::metadata::{MAX_WORKTREE_INDEX, ProjectMetadata};
use crate::msg;
use crate::testing::project::{Fixture, project_id};
use crate::testing::prompt::ScriptedPrompt;

#[test]
fn a_named_project_is_used_without_asking() -> Checked {
    let fixture = Fixture::new()?;
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
    let fixture = Fixture::new()?;
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
    let fixture = Fixture::new()?;
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
fn open_selects_a_project_and_index_without_reading_metadata() -> Checked {
    let fixture = Fixture::new()?;
    fixture.register("example-org/example-repo")?;
    fixture.register("other/other-repo")?;

    let (chosen, index) = open(
        &fixture.location,
        &msg!("select-open-heading"),
        &mut ScriptedPrompt::choosing_worktree(MAX_WORKTREE_INDEX),
        MAX_WORKTREE_INDEX,
    )
    .required_because("the combined open prompt returns both values")?;
    assert_eq!(chosen.display_id(), "example-org/example-repo");
    assert_eq!(index, MAX_WORKTREE_INDEX);
    Ok(())
}

#[test]
fn open_rejects_an_index_that_is_not_in_the_candidate_list() -> Checked {
    let fixture = Fixture::new()?;
    fixture.register("example-org/example-repo")?;

    let error = open(
        &fixture.location,
        &msg!("select-open-heading"),
        &mut ScriptedPrompt::choosing(7),
        MAX_WORKTREE_INDEX,
    )
    .refused_because("an invalid project selection is not opened")?;
    assert_eq!(error.first_id(), Some(ErrorId::SelectionUnresolved));
    Ok(())
}

#[test]
fn open_has_no_prompt_when_no_projects_are_registered() -> Checked {
    let fixture = Fixture::new()?;
    let mut prompt = ScriptedPrompt::choosing(0);
    let error = open(
        &fixture.location,
        &msg!("select-open-heading"),
        &mut prompt,
        MAX_WORKTREE_INDEX,
    )
    .refused_because("there is no project to open")?;
    assert_eq!(error.first_id(), Some(ErrorId::NoManagedProjects));
    assert!(prompt.asked.borrow().is_empty());
    Ok(())
}

#[test]
fn cancelling_the_prompt_changes_nothing() -> Checked {
    let fixture = Fixture::new()?;
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
    let fixture = Fixture::new()?;

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
    let fixture = Fixture::new()?;
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
    let fixture = Fixture::new()?;
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
    let fixture = Fixture::new()?;
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
    let fixture = Fixture::new()?;
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
            .map(crate::design::text::CommandLine::as_str)
            .collect::<Vec<_>>(),
        vec!["sbxm add git@github.com:Example-Org/Example-Repo.git"]
    );
    Ok(())
}

#[test]
fn a_registration_and_a_metadata_that_name_different_projects_are_not_reconciled() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;

    // registryが指すproject rootに、別の案件のmetadataが置かれている。どちらが正しいかを
    // sbxmは決められないため、片方を採用せずに食い違いそのものを報告する。
    let foreign = ProjectMetadata {
        repository: crate::testing::project::ssh_repository("other-org/other-repo")?,
        ..project.metadata.clone()
    };
    std::fs::write(
        project.paths.metadata_file(),
        crate::metadata::render(&foreign)?,
    )
    .required_because("write the metadata of another project")?;

    let candidate = one(
        &fixture.location,
        Some(&project_id("example-org/example-repo")?),
        &msg!("select-open-heading"),
        &mut ScriptedPrompt::choosing(0),
    )
    .required_because("the entry still names the project")?;

    let error = candidate
        .lock()
        .refused_because("a project that disagrees with its registration cannot be worked on")?;
    assert_eq!(error.first_id(), Some(ErrorId::ProjectInconsistent));
    let diagnostic = &error.diagnostics()[0];
    let argument = |key: &str| {
        diagnostic
            .description
            .args
            .iter()
            .find(|(name, _)| *name == key)
            .map(|(_, value)| value.as_str().to_string())
    };
    // 何と何が食い違っているかを、両方の clone URLで示す。
    assert_eq!(
        argument("observed").as_deref(),
        Some("git@github.com:other-org/other-repo.git")
    );
    assert_eq!(
        argument("expected").as_deref(),
        Some("git@github.com:example-org/example-repo.git")
    );
    assert!(
        diagnostic.remediation.is_some(),
        "the user is told how to resolve the disagreement"
    );
    Ok(())
}

#[test]
fn an_interrupted_prompt_is_a_cancel_and_any_other_read_failure_is_reported() {
    let canceled =
        crate::design::prompt::unreadable(&std::io::Error::from(std::io::ErrorKind::Interrupted));
    assert_eq!(canceled.exit_code(), ExitCode::Canceled);

    let unreadable =
        crate::design::prompt::unreadable(&std::io::Error::from(std::io::ErrorKind::BrokenPipe));
    assert_eq!(unreadable.first_id(), Some(ErrorId::PromptUnreadable));
    assert_ne!(unreadable.exit_code(), ExitCode::Canceled);
}

#[test]
fn a_registry_path_is_observed_before_it_is_read_or_written() -> Checked {
    let fixture = Fixture::new()?;
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

fn locked_for(fixture: &Fixture) -> Checked<Locked> {
    one(
        &fixture.location,
        Some(&project_id("example-org/example-repo")?),
        &msg!("select-open-heading"),
        &mut ScriptedPrompt::choosing(0),
    )
    .required_because("the project is managed")?
    .lock()
    .required_because("the project lock is free")
}

#[test]
fn shared_session_leases_are_held_by_more_than_one_open_at_once() -> Checked {
    let fixture = Fixture::new()?;
    fixture.register("example-org/example-repo")?;
    let locked = locked_for(&fixture)?;

    let first = locked
        .acquire_shared_session_lease()
        .required_because("the first sbxm open holds a shared lease")?;
    let second = locked
        .acquire_shared_session_lease()
        .required_because("a second sbxm open does not wait behind the first")?;
    drop(first);
    drop(second);
    Ok(())
}

#[test]
fn an_exclusive_session_lease_is_refused_while_a_session_is_open() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let locked = locked_for(&fixture)?;

    let session = locked
        .acquire_shared_session_lease()
        .required_because("an sbxm open session is active")?;

    let error = locked
        .acquire_exclusive_session_lease()
        .refused_because("a normal rebuild/destroy must not run while a session is open")?;
    assert_eq!(error.first_id(), Some(ErrorId::OpenSessionActive));
    let diagnostic = &error.diagnostics()[0];
    assert_eq!(
        diagnostic
            .description
            .args
            .iter()
            .find(|(key, _)| *key == "project")
            .map(|(_, value)| value.as_str()),
        Some(project.metadata.display_id().as_str()),
        "the project the open session belongs to is named"
    );
    assert!(
        diagnostic.remediation.is_some(),
        "the user is told to wait, not to delete the lease file"
    );

    drop(session);
    locked
        .acquire_exclusive_session_lease()
        .required_because("once the session closes, rebuild/destroy can proceed")?;
    Ok(())
}

#[test]
fn a_stale_session_lease_file_alone_does_not_count_as_an_open_session() -> Checked {
    // fileの存在ではなく、OS lockの成否だけが根拠になる。
    let fixture = Fixture::new()?;
    fixture.register("example-org/example-repo")?;
    let locked = locked_for(&fixture)?;
    let lease_file = locked.paths.session_lease_file();
    std::fs::write(&lease_file, b"stale")
        .required_because("leave a stale file behind, held by nobody")?;
    std::fs::set_permissions(
        &lease_file,
        std::os::unix::fs::PermissionsExt::from_mode(crate::paths::PRIVATE_FILE_MODE),
    )
    .required_because("match the mode a real lock file would have")?;

    locked
        .acquire_exclusive_session_lease()
        .required_because("a stale file with no OS lock held blocks nothing")?;
    Ok(())
}
