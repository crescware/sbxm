use crate::commands::apply::Scope;
use crate::diagnostics::ErrorId;
use crate::metadata::{self};
use crate::paths::ProjectPaths;
use crate::project::{SandboxLayout, SandboxName};

use crate::testing::outcome::{Checked, Refused, Required};

use super::{super::fake::*, *};
use crate::design::SilentProgress;
use crate::hash::sha256_hex;
use crate::metadata::RebuildIntent;
use crate::paths::{LOCK_TIMEOUT, PRIVATE_FILE_MODE, PathScope};
use crate::testing::prompt::ScriptedPrompt;
use crate::testing::value::DIGEST;
use std::os::unix::fs::PermissionsExt;

/// 既存testは宣言fileの配置を確かめる。
const FILES_ONLY: Scope = Scope {
    files: true,
    worktrees: None,
};

#[test]
fn asking_for_worktrees_leaves_the_declared_files_alone() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let source = dir.path().join("declared.yaml");
    std::fs::write(&source, b"declared = true\n").required()?;

    let (_home, location, parent, config, workspace_root) = setup(vec![declaration(&source)?])?;
    let paths = write_metadata(&location, &parent, None)?;
    let host = FakeSbx::listing(&listing(&workspace_root, "running")?).holding_repository()?;

    let output = run(
        Target {
            location: &location,
            requested: Some(&project()?),
            prompt: &mut ScriptedPrompt::choosing(0),
        },
        &config,
        WORKTREES_ONLY,
        &host,
        &workspace_root,
        &mut SilentProgress,
    )
    .required_because("worktrees are applied on their own")?;

    assert_eq!(output.worktrees, Some(3));
    assert!(output.files.is_empty());
    // 宣言fileの配置は既存のfileを上書きする。名指していない対象へは触れない。
    assert!(
        !host.ran("cp --follow-link"),
        "the declared files were not asked for"
    );

    let stored = metadata::load(&paths)
        .required()?
        .required_because("present")?;
    assert_eq!(stored.provisioning.requested_worktrees, 3);
    Ok(())
}

#[test]
fn asking_for_the_number_the_project_already_targets_rewrites_nothing() -> Checked {
    // 目標が変わらない指定はmetadataを書き換えない。同じ値の書き戻しでも、
    // 保存済みの宣言に触れる理由にはならない。
    let (_home, location, parent, config, workspace_root) = setup(Vec::new())?;
    let paths = write_metadata(&location, &parent, None)?;
    let before = std::fs::read_to_string(paths.metadata_file()).required()?;
    let host = FakeSbx::listing(&listing(&workspace_root, "running")?).holding_repository()?;

    let scope = Scope {
        files: false,
        worktrees: Some(1),
    };
    let output = run(
        Target {
            location: &location,
            requested: Some(&project()?),
            prompt: &mut ScriptedPrompt::choosing(0),
        },
        &config,
        scope,
        &host,
        &workspace_root,
        &mut SilentProgress,
    )
    .required_because("the target is already what was asked for")?;

    assert_eq!(output.worktrees, Some(1));
    assert_eq!(
        std::fs::read_to_string(paths.metadata_file()).required()?,
        before,
        "an unchanged target leaves the stored declaration untouched"
    );
    Ok(())
}

#[test]
fn a_start_ref_the_sandbox_cannot_resolve_stops_the_run_before_any_worktree_is_made() -> Checked {
    // 起点はSandboxの中のGit自身に確かめさせる。解決できない起点のまま、
    // どこから生えたか分からないworktreeを作らない。
    let (_home, location, parent, config, workspace_root) = setup(Vec::new())?;
    write_metadata(&location, &parent, None)?;
    let git_dir = SandboxLayout::new(&canonical()?).bare_git_dir();
    let host = FakeSbx::listing(&listing(&workspace_root, "running")?)
        .holding_repository()?
        .failing(&format!(
            "git --git-dir {git_dir} show-ref --verify --quiet refs/remotes/origin/main"
        ));

    let error = run(
        Target {
            location: &location,
            requested: Some(&project()?),
            prompt: &mut ScriptedPrompt::choosing(0),
        },
        &config,
        WORKTREES_ONLY,
        &host,
        &workspace_root,
        &mut SilentProgress,
    )
    .refused_because("the start ref is not resolvable inside the sandbox")?;

    assert_eq!(error.first_id(), Some(ErrorId::StartRefUnresolved));
    assert!(
        !host.ran("worktree add"),
        "no worktree is created from an unresolved start ref: {:?}",
        host.calls()
    );
    Ok(())
}

#[test]
fn a_failure_while_making_the_worktrees_keeps_the_raised_target() -> Checked {
    // 目標本数の引き上げは作業の前にcommitする。途中で失敗した実行は成功を報告せず、
    // 同じcommandでやり直せる状態を残す。
    let (_home, location, parent, config, workspace_root) = setup(Vec::new())?;
    let paths = write_metadata(&location, &parent, None)?;
    let git_dir = SandboxLayout::new(&canonical()?).bare_git_dir();
    let host = FakeSbx::listing(&listing(&workspace_root, "running")?)
        .holding_repository()?
        .failing(&format!(
            "git --git-dir {git_dir} rev-parse refs/remotes/origin/main"
        ));

    let error = run(
        Target {
            location: &location,
            requested: Some(&project()?),
            prompt: &mut ScriptedPrompt::choosing(0),
        },
        &config,
        WORKTREES_ONLY,
        &host,
        &workspace_root,
        &mut SilentProgress,
    )
    .refused_because("the commit the worktrees start from cannot be read")?;

    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandFailed));
    let stored = metadata::load(&paths)
        .required()?
        .required_because("present")?;
    assert_eq!(
        stored.provisioning.requested_worktrees, 3,
        "the raised target stays so the run can be repeated"
    );
    Ok(())
}

const REALISTIC_DF: &str = "Filesystem     1024-blocks      Used Available Capacity Mounted on\noverlay          20466256  14502976   4898320       75% /\n";

#[test]
fn a_bare_repository_fetch_failure_carries_the_disk_state_at_that_moment() -> Checked {
    let (_home, location, parent, config, workspace_root) = setup(Vec::new())?;
    write_metadata(&location, &parent, None)?;
    let git_dir = SandboxLayout::new(&canonical()?).bare_git_dir();
    let host = FakeSbx::listing(&listing(&workspace_root, "running")?)
        .holding_repository()?
        .failing(&format!(
            "git --git-dir {git_dir} fetch --prune --progress origin"
        ))
        .answering("df -Pk /", REALISTIC_DF);

    let mark = host.calls().len();
    let error = run(
        Target {
            location: &location,
            requested: Some(&project()?),
            prompt: &mut ScriptedPrompt::choosing(0),
        },
        &config,
        WORKTREES_ONLY,
        &host,
        &workspace_root,
        &mut SilentProgress,
    )
    .refused_because("the bare repository cannot be refreshed")?;

    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandFailed));
    let facts = &error.diagnostics()[0].facts;
    assert_eq!(facts.len(), 3, "{facts:?}");
    let since = &host.calls()[mark..];
    assert_eq!(
        since
            .iter()
            .filter(|call| call.contains(&"df".to_string()))
            .count(),
        1,
        "exactly one disk check per failure: {since:?}"
    );
    Ok(())
}

#[test]
fn a_successful_apply_never_asks_for_disk_usage() -> Checked {
    let (_home, location, parent, config, workspace_root) = setup(Vec::new())?;
    write_metadata(&location, &parent, None)?;
    let host = FakeSbx::listing(&listing(&workspace_root, "running")?)
        .holding_repository()?
        .answering("df -Pk /", REALISTIC_DF);

    run(
        Target {
            location: &location,
            requested: Some(&project()?),
            prompt: &mut ScriptedPrompt::choosing(0),
        },
        &config,
        WORKTREES_ONLY,
        &host,
        &workspace_root,
        &mut SilentProgress,
    )
    .required_because("apply succeeds")?;

    assert!(
        !host.ran("df -Pk"),
        "a successful run never checks disk usage: {:?}",
        host.calls()
    );
    Ok(())
}

#[test]
fn a_number_below_what_the_project_has_is_refused() -> Checked {
    let (_home, location, parent, config, workspace_root) = setup(Vec::new())?;
    let paths = write_metadata(&location, &parent, None)?;
    let mut metadata = metadata::load(&paths)
        .required()?
        .required_because("present")?;
    metadata.provisioning.requested_worktrees = 3;
    metadata::update(&paths, &metadata).required()?;
    let host = FakeSbx::listing(&listing(&workspace_root, "running")?);

    let scope = Scope {
        files: false,
        worktrees: Some(2),
    };
    let error = run(
        Target {
            location: &location,
            requested: Some(&project()?),
            prompt: &mut ScriptedPrompt::choosing(0),
        },
        &config,
        scope,
        &host,
        &workspace_root,
        &mut SilentProgress,
    )
    .refused_because("removing a worktree deletes what is checked out in it")?;
    assert_eq!(error.first_id(), Some(ErrorId::WorktreesNotReducible));

    let stored = metadata::load(&paths)
        .required()?
        .required_because("present")?;
    assert_eq!(
        stored.provisioning.requested_worktrees, 3,
        "a refused run leaves the target where it was"
    );
    Ok(())
}

#[test]
fn a_running_project_gets_the_declared_files_replaced() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let source = dir.path().join("declared.yaml");
    std::fs::write(&source, b"declared = true\n").required()?;
    let _ = sha256_hex(b"declared = true\n");

    let (_home, location, parent, config, workspace_root) = setup(vec![declaration(&source)?])?;
    write_metadata(&location, &parent, None)?;
    let host = FakeSbx::listing(&listing(&workspace_root, "running")?);

    let output = run(
        Target {
            location: &location,
            requested: Some(&project()?),
            prompt: &mut ScriptedPrompt::choosing(0),
        },
        &config,
        FILES_ONLY,
        &host,
        &workspace_root,
        &mut SilentProgress,
    )
    .required_because("sync")?;
    assert_eq!(output.project, "Example-Org/Example-Repo");
    assert_eq!(output.files.len(), 1);
    assert!(host.ran("cp --follow-link"));
    Ok(())
}

#[test]
fn the_project_lock_is_held_while_the_files_are_replaced() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let source = dir.path().join("declared.yaml");
    std::fs::write(&source, b"declared = true\n").required()?;

    let (_home, location, parent, config, workspace_root) = setup(vec![declaration(&source)?])?;
    let paths = write_metadata(&location, &parent, None)?;
    let host =
        FakeSbx::listing(&listing(&workspace_root, "running")?).watching_lock(paths.lock_file());

    run(
        Target {
            location: &location,
            requested: Some(&project()?),
            prompt: &mut ScriptedPrompt::choosing(0),
        },
        &config,
        FILES_ONLY,
        &host,
        &workspace_root,
        &mut SilentProgress,
    )
    .required_because("sync")?;

    assert_eq!(
        *host.lock_was_free.borrow(),
        Some(false),
        "another run must not reach the same sandbox while files are being replaced"
    );
    assert_eq!(
        std::fs::metadata(paths.lock_file())
            .required()?
            .permissions()
            .mode()
            & 0o777,
        PRIVATE_FILE_MODE
    );

    // lockはworkflow終了後に解放され、lock file自体は残る。
    crate::paths::acquire_exclusive_lock(
        &paths.lock_file(),
        LOCK_TIMEOUT,
        PRIVATE_FILE_MODE,
        PathScope::ProjectPath,
    )
    .required_because("the lock is released when the workflow ends")?;
    Ok(())
}

#[test]
fn a_project_that_is_not_managed_gets_no_lock_file() -> Checked {
    let (_home, location, parent, config, workspace_root) = setup(Vec::new())?;
    let host = FakeSbx::listing(r#"{"sandboxes":[]}"#);

    run(
        Target {
            location: &location,
            requested: Some(&project()?),
            prompt: &mut ScriptedPrompt::choosing(0),
        },
        &config,
        FILES_ONLY,
        &host,
        &workspace_root,
        &mut SilentProgress,
    )
    .refused_because("nothing to place")?;

    let paths = ProjectPaths::derive(&parent, &canonical()?);
    assert!(
        !paths.lock_file().exists(),
        "an unmanaged project is not given a lock file"
    );
    Ok(())
}

#[test]
fn nothing_else_in_the_project_is_touched() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let source = dir.path().join("declared.yaml");
    std::fs::write(&source, b"declared = true\n").required()?;
    let (_home, location, parent, config, workspace_root) = setup(vec![declaration(&source)?])?;
    let paths = write_metadata(&location, &parent, None)?;
    let before = std::fs::read_to_string(paths.metadata_file()).required()?;
    let host = FakeSbx::listing(&listing(&workspace_root, "running")?);

    run(
        Target {
            location: &location,
            requested: Some(&project()?),
            prompt: &mut ScriptedPrompt::choosing(0),
        },
        &config,
        FILES_ONLY,
        &host,
        &workspace_root,
        &mut SilentProgress,
    )
    .required_because("sync")?;

    for forbidden in [
        "build",
        "image save",
        "template load",
        "worktree add",
        "clone",
    ] {
        assert!(
            !host.ran(forbidden),
            "sync-files must not run {forbidden}: {:?}",
            host.calls()
        );
    }
    assert_eq!(
        std::fs::read_to_string(paths.metadata_file()).required()?,
        before,
        "the metadata is read-only for sync-files"
    );
    Ok(())
}

#[test]
fn a_stopped_sandbox_is_not_started_and_the_user_is_sent_to_open() -> Checked {
    let (_home, location, parent, config, workspace_root) = setup(Vec::new())?;
    write_metadata(&location, &parent, None)?;
    let host = FakeSbx::listing(&listing(&workspace_root, "stopped")?);

    let error = run(
        Target {
            location: &location,
            requested: Some(&project()?),
            prompt: &mut ScriptedPrompt::choosing(0),
        },
        &config,
        FILES_ONLY,
        &host,
        &workspace_root,
        &mut SilentProgress,
    )
    .refused_because("a stopped sandbox is not started implicitly")?;
    assert_eq!(error.first_id(), Some(ErrorId::SandboxNotRunning));
    assert_eq!(
        error.diagnostics()[0]
            .remediation
            .as_ref()
            .and_then(|remediation| remediation.explanation.first())
            .map(|message| message.id),
        Some("remediation-sandbox-not-running")
    );
    // 起動commandは`exec <name> -- /bin/true`であるため、一覧以外が走っていないことで見る。
    let beyond_listing: Vec<Vec<String>> = host
        .calls()
        .into_iter()
        .filter(|args| args.first().map(String::as_str) != Some("ls"))
        .collect();
    assert!(
        beyond_listing.is_empty(),
        "a stopped sandbox is not started implicitly: {beyond_listing:?}"
    );
    Ok(())
}

#[test]
fn a_project_that_is_not_managed_or_not_built_is_refused() -> Checked {
    let (_home, location, parent, config, workspace_root) = setup(Vec::new())?;
    let host = FakeSbx::listing(r#"{"sandboxes":[]}"#);
    let error = run(
        Target {
            location: &location,
            requested: Some(&project()?),
            prompt: &mut ScriptedPrompt::choosing(0),
        },
        &config,
        FILES_ONLY,
        &host,
        &workspace_root,
        &mut SilentProgress,
    )
    .refused_because("an unregistered project has nowhere to place files")?;
    assert_eq!(error.first_id(), Some(ErrorId::ProjectNotManaged));

    write_metadata(&location, &parent, None)?;
    let error = run(
        Target {
            location: &location,
            requested: Some(&project()?),
            prompt: &mut ScriptedPrompt::choosing(0),
        },
        &config,
        FILES_ONLY,
        &host,
        &workspace_root,
        &mut SilentProgress,
    )
    .refused_because("a registered project without a sandbox has nowhere to place files")?;
    assert_eq!(error.first_id(), Some(ErrorId::SandboxNotCreated));
    Ok(())
}

#[test]
fn a_rebuild_in_progress_places_nothing() -> Checked {
    let (_home, location, parent, config, workspace_root) = setup(Vec::new())?;
    write_metadata(
        &location,
        &parent,
        Some(RebuildIntent {
            target_dockerfile_sha256: "2".repeat(64),
            previous_dockerfile_sha256: DIGEST.into(),
        }),
    )?;
    let host = FakeSbx::listing(&listing(&workspace_root, "running")?);

    let error = run(
        Target {
            location: &location,
            requested: Some(&project()?),
            prompt: &mut ScriptedPrompt::choosing(0),
        },
        &config,
        FILES_ONLY,
        &host,
        &workspace_root,
        &mut SilentProgress,
    )
    .refused_because("a half-switched sandbox is not the target of a placement")?;
    assert_eq!(error.first_id(), Some(ErrorId::RebuildIntentPending));
    assert!(host.calls().is_empty(), "nothing is asked of the runtime");
    Ok(())
}

#[test]
fn a_sandbox_that_belongs_to_another_project_is_refused() -> Checked {
    let (_home, location, parent, config, workspace_root) = setup(Vec::new())?;
    write_metadata(&location, &parent, None)?;
    let name = SandboxName::derive(&canonical()?);
    let host = FakeSbx::listing(&format!(
        r#"{{"sandboxes":[{{"name":"{name}","status":"running","workspaces":["/tmp/elsewhere"]}}]}}"#
    ));

    let error = run(
        Target {
            location: &location,
            requested: Some(&project()?),
            prompt: &mut ScriptedPrompt::choosing(0),
        },
        &config,
        FILES_ONLY,
        &host,
        &workspace_root,
        &mut SilentProgress,
    )
    .refused_because("a sandbox that cannot be identified is not written to")?;
    assert_eq!(error.first_id(), Some(ErrorId::SandboxUnusable));
    Ok(())
}
