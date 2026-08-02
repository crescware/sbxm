use crate::diagnostics::ErrorId;
use crate::metadata::{self};

use crate::testing::outcome::{Checked, Refused, Required};

use super::*;
use crate::metadata::CreationMode;
use crate::testing::repository::*;
use crate::testing::sandbox::InnerCommandSandbox;

#[test]
fn an_attached_project_resolves_the_remote_default_branch_and_records_it() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let paths = project_paths(dir.path())?;
    let git_dir = layout()?.bare_git_dir();
    let host = InnerCommandSandbox::new().answering(
        &format!("git --git-dir {git_dir} ls-remote --symref origin HEAD"),
        "ref: refs/heads/main\tHEAD\n9f5b1c\tHEAD\n",
    );

    let mut project = metadata(CreationMode::Attached, None, 1)?;
    metadata::create(&paths, &project).required_because("write the metadata")?;

    let branch = resolve_start_ref(&host, "sbxm-example", &layout()?, &paths, &mut project)
        .required_because("resolve")?;
    assert_eq!(branch, "main");
    assert_eq!(project.provisioning.start_ref.as_deref(), Some("main"));

    let stored = metadata::load(&paths)
        .required()?
        .required_because("present")?;
    assert_eq!(
        stored.provisioning.start_ref.as_deref(),
        Some("main"),
        "the resolved branch is written before any worktree is made"
    );
    assert!(host.ran("git check-ref-format --branch main"));
    assert!(host.ran("show-ref --verify --quiet refs/remotes/origin/main"));
    Ok(())
}

#[test]
fn a_head_that_points_at_something_other_than_a_branch_is_not_a_start_point() -> Checked {
    let git_dir = layout()?.bare_git_dir();
    let cases = [
        // branch以外を指すHEAD。
        "ref: refs/tags/v1.0.0\tHEAD\n9f5b1c\tHEAD\n",
        // branchのpathだが、名前として渡せない綴り。
        "ref: refs/heads/-oops\tHEAD\n9f5b1c\tHEAD\n",
    ];

    for answer in cases {
        let dir = tempfile::tempdir().required()?;
        let paths = project_paths(dir.path())?;
        let host = InnerCommandSandbox::new().answering(
            &format!("git --git-dir {git_dir} ls-remote --symref origin HEAD"),
            answer,
        );

        let mut project = metadata(CreationMode::Attached, None, 1)?;
        metadata::create(&paths, &project).required_because("write the metadata")?;

        let error = resolve_start_ref(&host, "sbxm-example", &layout()?, &paths, &mut project)
            .refused_because("only a branch can be the start point")?;
        assert_eq!(error.first_id(), Some(ErrorId::ExternalOutputUnparseable));

        let stored = metadata::load(&paths)
            .required()?
            .required_because("present")?;
        assert_eq!(
            stored.provisioning.start_ref, None,
            "nothing is adopted from a HEAD that names no branch"
        );
        assert!(
            !host.ran("check-ref-format"),
            "a name that is not a branch is never handed to git: {:?}",
            host.calls()
        );
    }
    Ok(())
}

#[test]
fn a_symref_line_that_names_no_reference_does_not_end_the_search() -> Checked {
    // `ref:`だけの行はrefを示していない。そこで打ち切ると、続く行が答えを持って
    // いても「HEADがbranchを示さない」と告げることになる。
    let dir = tempfile::tempdir().required()?;
    let paths = project_paths(dir.path())?;
    let git_dir = layout()?.bare_git_dir();
    let host = InnerCommandSandbox::new().answering(
        &format!("git --git-dir {git_dir} ls-remote --symref origin HEAD"),
        "ref:\nref: refs/heads/main\tHEAD\n9f5b1c\tHEAD\n",
    );

    let mut project = metadata(CreationMode::Attached, None, 1)?;
    metadata::create(&paths, &project).required_because("write the metadata")?;

    let branch = resolve_start_ref(&host, "sbxm-example", &layout()?, &paths, &mut project)
        .required_because("resolve")?;
    assert_eq!(branch, "main");
    Ok(())
}

#[test]
fn a_question_the_host_could_not_ask_is_not_read_as_an_answer_about_the_branch() -> Checked {
    // 起点の決定は3つの問い合わせから成る。どれも「届かなかった」ことを「そう
    // 答えられた」に読み替えると、branch名やrefについて観測していない断定をする。
    let git_dir = layout()?.bare_git_dir();
    let cases = [
        (
            CreationMode::Attached,
            None,
            format!("git --git-dir {git_dir} ls-remote --symref origin HEAD"),
        ),
        (
            CreationMode::Detached,
            Some("develop"),
            "git check-ref-format --branch develop".to_string(),
        ),
        (
            CreationMode::Detached,
            Some("develop"),
            format!(
                "git --git-dir {git_dir} show-ref --verify --quiet refs/remotes/origin/develop"
            ),
        ),
    ];

    for (mode, declared, step) in cases {
        let dir = tempfile::tempdir().required()?;
        let paths = project_paths(dir.path())?;
        let host = InnerCommandSandbox::new()
            .answering(
                &format!("git --git-dir {git_dir} ls-remote --symref origin HEAD"),
                "ref: refs/heads/develop\tHEAD\n",
            )
            .timing_out(&step);

        let mut project = metadata(mode, declared, 1)?;
        metadata::create(&paths, &project).required_because("write the metadata")?;

        let error = resolve_start_ref(&host, "sbxm-example", &layout()?, &paths, &mut project)
            .refused_because("a question that went unanswered stops the run")?;
        assert_eq!(
            error.first_id(),
            Some(ErrorId::ExternalCommandTimeout),
            "{step} was reported as something other than the host failure it was"
        );

        let recorded = metadata::load(&paths)
            .required()?
            .required_because("present")?;
        assert_eq!(
            recorded.provisioning.start_ref.as_deref(),
            declared,
            "the target configuration keeps the branch it declared, if any"
        );
    }
    Ok(())
}

#[test]
fn the_start_branch_is_judged_again_by_git_inside_the_sandbox() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let paths = project_paths(dir.path())?;
    // hostのvalidationは通るが、gitがbranch名として受け付けない値。
    let host = InnerCommandSandbox::new().failing("git check-ref-format --branch feature..login");

    let mut project = metadata(CreationMode::Detached, Some("feature..login"), 1)?;
    metadata::create(&paths, &project).required_because("write the metadata")?;

    let error = resolve_start_ref(&host, "sbxm-example", &layout()?, &paths, &mut project)
        .refused_because("git has the final say on what is a branch name")?;
    assert_eq!(error.first_id(), Some(ErrorId::InvalidBranchName));
    assert!(
        !host.ran("show-ref"),
        "a name git refuses is never looked up: {:?}",
        host.calls()
    );
    Ok(())
}

#[test]
fn a_resolved_branch_that_git_refuses_is_not_recorded() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let paths = project_paths(dir.path())?;
    let git_dir = layout()?.bare_git_dir();
    let host = InnerCommandSandbox::new()
        .answering(
            &format!("git --git-dir {git_dir} ls-remote --symref origin HEAD"),
            "ref: refs/heads/main\tHEAD\n",
        )
        .failing("git check-ref-format --branch main");

    let mut project = metadata(CreationMode::Attached, None, 1)?;
    metadata::create(&paths, &project).required_because("write the metadata")?;

    let error = resolve_start_ref(&host, "sbxm-example", &layout()?, &paths, &mut project)
        .refused_because("an unusable name is not adopted")?;
    assert_eq!(error.first_id(), Some(ErrorId::InvalidBranchName));

    let stored = metadata::load(&paths)
        .required()?
        .required_because("present")?;
    assert_eq!(
        stored.provisioning.start_ref, None,
        "the target configuration keeps waiting for a branch it can use"
    );
    Ok(())
}

#[test]
fn a_start_branch_that_has_no_remote_tracking_ref_stops_the_run() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let paths = project_paths(dir.path())?;
    let git_dir = layout()?.bare_git_dir();
    let host = InnerCommandSandbox::new().failing(&format!(
        "git --git-dir {git_dir} show-ref --verify --quiet refs/remotes/origin/develop"
    ));

    let mut project = metadata(CreationMode::Detached, Some("develop"), 1)?;
    metadata::create(&paths, &project).required_because("write the metadata")?;

    let error = resolve_start_ref(&host, "sbxm-example", &layout()?, &paths, &mut project)
        .refused_because("a branch that is not on the remote cannot be a start point")?;
    assert_eq!(error.first_id(), Some(ErrorId::StartRefUnresolved));
    Ok(())
}
