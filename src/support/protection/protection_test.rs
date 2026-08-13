use crate::diagnostics::{ErrorId, Result};
use crate::project::SandboxLayout;

use crate::testing::outcome::{Checked, Refused, Required};

use super::*;
use crate::testing::host::FakeSbx;
use crate::testing::project::{Fixture, Registered};
use crate::testing::protection::clean_host;
use crate::testing::value::COMMIT;

fn inspect_with(
    host: &FakeSbx,
    project: &Registered,
    unmanaged: Unmanaged,
    workspace_root: &std::path::Path,
) -> Result<Report> {
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    inspect(
        host,
        &project.sandbox,
        workspace_root,
        &layout,
        &project.metadata,
        unmanaged,
    )
}

#[test]
fn a_clean_managed_worktree_passes_and_is_reported() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let host = clean_host(&fixture, &project)?;

    let protection = inspect_with(&host, &project, Unmanaged::Refused, &fixture.workspace_root)
        .required_because("a clean worktree passes")?;
    assert_eq!(
        protection.worktrees,
        vec![WorktreeReport {
            relative: "example-repo.tree-0".to_string(),
            kind: Kind::Managed,
            mode: Mode::Attached,
            head: COMMIT.to_string(),
            branch: Some("main".to_string()),
            remote: Remote::Pushed,
        }]
    );
    Ok(())
}

#[test]
fn work_that_is_not_committed_or_not_pushed_stops_the_run() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let name = project.sandbox.as_str();
    let managed = format!("{}/example-repo.tree-0", layout.bare_root());

    let dirty = clean_host(&fixture, &project)?.answering(
        &format!("exec {name} -- git -C {managed} status --porcelain=v2 -z --untracked-files=all"),
        0,
        "? untracked.txt\0",
    );
    let unpushed = clean_host(&fixture, &project)?.answering(
        &format!("exec {name} -- git -C {managed} rev-list --count origin/main..HEAD"),
        0,
        "2\n",
    );
    let no_upstream = clean_host(&fixture, &project)?.answering(
            &format!(
                "exec {name} -- git -C {managed} rev-parse --abbrev-ref --symbolic-full-name @{{upstream}}"
            ),
            1,
            "",
        );
    let in_progress = clean_host(&fixture, &project)?.answering(
        &format!("exec {name} -- test -e {managed}/.git/MERGE_HEAD"),
        0,
        "",
    );

    for host in [dirty, unpushed, no_upstream, in_progress] {
        let error = inspect_with(&host, &project, Unmanaged::Refused, &fixture.workspace_root)
            .refused_because("unsaved work is never destroyed")?;
        assert_eq!(error.first_id(), Some(ErrorId::UnsavedWork));
    }
    Ok(())
}

#[test]
fn a_check_that_could_not_run_is_never_read_as_a_pass() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let name = project.sandbox.as_str();
    let managed = format!("{}/example-repo.tree-0", layout.bare_root());

    // `sbx exec`が内側のcommandを起動できなかったことを示す終了status。
    let marker = clean_host(&fixture, &project)?.answering(
        &format!("exec {name} -- test -e {managed}/.git/MERGE_HEAD"),
        126,
        "",
    );
    let head = clean_host(&fixture, &project)?.answering(
        &format!("exec {name} -- git -C {managed} symbolic-ref --quiet --short HEAD"),
        127,
        "",
    );
    let upstream = clean_host(&fixture, &project)?.answering(
            &format!(
                "exec {name} -- git -C {managed} rev-parse --abbrev-ref --symbolic-full-name @{{upstream}}"
            ),
            125,
            "",
        );

    let cases = [
        (marker, format!("{managed}/.git/MERGE_HEAD"), 126),
        (head, "HEAD".to_string(), 127),
        (upstream, "@{upstream}".to_string(), 125),
    ];
    for (host, subject, code) in cases {
        let error = inspect_with(&host, &project, Unmanaged::Allowed, &fixture.workspace_root)
            .refused_because("a check that did not answer never means the worktree is safe")?;
        assert_eq!(error.first_id(), Some(ErrorId::SandboxCheckUnobservable));
        let diagnostic = &error.diagnostics()[0];
        assert_eq!(
            diagnostic.description.id,
            "error-sandbox-check-unobservable"
        );
        assert_eq!(
            diagnostic.description.args,
            vec![
                ("subject", subject),
                ("exit_status", format!("exit status: {code}"))
            ],
            "the diagnostic names the check that did not answer"
        );
        let external = diagnostic
            .external
            .as_ref()
            .required_because("the runtime's own failure is kept with the diagnostic")?;
        assert_eq!(external.program, "sbx");
        assert_eq!(external.exit_status, format!("exit status: {code}"));
    }
    Ok(())
}

#[test]
fn an_unmanaged_worktree_is_refused_for_rebuild_and_examined_for_destroy() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let name = project.sandbox.as_str();
    let managed = format!("{}/example-repo.tree-0", layout.bare_root());
    let extra = format!("{}/agent-scratch", layout.bare_root());

    let host = clean_host(&fixture, &project)?
            .answering(
                &format!(
                    "exec {name} -- git --git-dir {} worktree list --porcelain -z",
                    layout.bare_git_dir()
                ),
                0,
                &format!(
                    "worktree {}\0bare\0\0worktree {managed}\0branch refs/heads/main\0\0worktree {extra}\0detached\0\0",
                    layout.bare_root()
                ),
            )
            .answering(
                &format!("exec {name} -- git -C {extra} status --porcelain=v2 -z --untracked-files=all"),
                0,
                "",
            )
            .answering(
                &format!("exec {name} -- git -C {extra} rev-parse --git-dir"),
                0,
                &format!("{extra}/.git\n"),
            )
            .answering(
                &format!("exec {name} -- git -C {extra} rev-parse HEAD"),
                0,
                &format!("{COMMIT}\n"),
            )
            .answering(
                &format!("exec {name} -- git -C {extra} symbolic-ref --quiet --short HEAD"),
                1,
                "",
            )
            .answering(
                &format!("exec {name} -- git -C {extra} rev-list --count HEAD --not --remotes=origin"),
                0,
                "0\n",
            )
            .answering(&format!("exec {name} -- test -e {extra}/.git/MERGE_HEAD"), 1, "")
            .answering(&format!("exec {name} -- test -e {extra}/.git/CHERRY_PICK_HEAD"), 1, "")
            .answering(&format!("exec {name} -- test -e {extra}/.git/REVERT_HEAD"), 1, "")
            .answering(&format!("exec {name} -- test -e {extra}/.git/BISECT_LOG"), 1, "")
            .answering(&format!("exec {name} -- test -e {extra}/.git/rebase-merge"), 1, "")
            .answering(&format!("exec {name} -- test -e {extra}/.git/rebase-apply"), 1, "");

    let error = inspect_with(&host, &project, Unmanaged::Refused, &fixture.workspace_root)
        .refused_because("rebuild cannot recreate a worktree it does not know about")?;
    assert_eq!(error.first_id(), Some(ErrorId::UnmanagedWorktreePresent));

    let protection = inspect_with(&host, &project, Unmanaged::Allowed, &fixture.workspace_root)
        .required_because("destroy examines it under the same rules")?;
    assert_eq!(protection.worktrees.len(), 2);
    assert_eq!(protection.worktrees[1].kind, Kind::Unmanaged);
    assert_eq!(protection.worktrees[1].remote, Remote::Reachable);
    Ok(())
}

#[test]
fn a_worktree_that_is_not_an_artifact_of_this_project_is_not_reported_as_unsaved_work() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let name = project.sandbox.as_str();
    let listing = format!(
        "exec {name} -- git --git-dir {} worktree list --porcelain -z",
        layout.bare_git_dir()
    );

    // bare rootの外を指すworktree。
    let outside = clean_host(&fixture, &project)?.answering(
        &listing,
        0,
        &format!(
            "worktree {}\0bare\0\0worktree /home/agent/elsewhere\0branch refs/heads/main\0\0",
            layout.bare_root()
        ),
    );
    let error = inspect_with(
        &outside,
        &project,
        Unmanaged::Allowed,
        &fixture.workspace_root,
    )
    .refused_because("a path outside the repository is a security refusal")?;
    assert_eq!(error.first_id(), Some(ErrorId::WorktreeOutsideRepository));
    Ok(())
}

#[test]
fn a_sandbox_whose_git_lists_no_worktree_has_nothing_to_lose() -> Checked {
    // 構築や再構築が途中で終わったSandboxには、checkoutされた作業が存在しない。
    // 宣言との食い違いを理由に止めると、作り直す手段がなくなる。
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let name = project.sandbox.as_str();
    let listing = format!(
        "exec {name} -- git --git-dir {} worktree list --porcelain -z",
        layout.bare_git_dir()
    );

    let empty = clean_host(&fixture, &project)?.answering(
        &listing,
        0,
        &format!("worktree {}\0bare\0\0", layout.bare_root()),
    );
    let protection = inspect_with(
        &empty,
        &project,
        Unmanaged::Refused,
        &fixture.workspace_root,
    )
    .required_because("a sandbox holding no worktree can be replaced")?;
    assert!(protection.worktrees.is_empty());
    Ok(())
}

#[test]
fn a_detached_head_that_no_remote_reaches_stops_the_run() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let name = project.sandbox.as_str();
    let managed = format!("{}/example-repo.tree-0", layout.bare_root());

    let host = clean_host(&fixture, &project)?
        .answering(
            &format!("exec {name} -- git -C {managed} symbolic-ref --quiet --short HEAD"),
            1,
            "",
        )
        .answering(
            &format!(
                "exec {name} -- git -C {managed} rev-list --count HEAD --not --remotes=origin"
            ),
            0,
            "3\n",
        );

    let error = inspect_with(&host, &project, Unmanaged::Allowed, &fixture.workspace_root)
        .refused_because("commits no remote holds are not thrown away")?;
    assert_eq!(error.first_id(), Some(ErrorId::UnsavedWork));
    Ok(())
}

#[test]
fn a_sandbox_without_the_shared_repository_has_nothing_to_lose() -> Checked {
    // 構築が途中で終わったSandboxには、この案件の作業が1件もない。worktreeが
    // 観測できないことを、失うものがある徴候として読まない。
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let name = project.sandbox.as_str();
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let host = clean_host(&fixture, &project)?.answering(
        &format!(
            "exec {name} -- sh -c {BARE_GIT_DIR_PROBE} sh {}",
            layout.bare_git_dir()
        ),
        1,
        "probed",
    );

    let protection = inspect_with(&host, &project, Unmanaged::Refused, &fixture.workspace_root)
        .required_because("a sandbox that holds no repository can be replaced")?;
    assert!(protection.worktrees.is_empty());
    Ok(())
}

#[test]
fn a_workspace_directory_missing_on_the_host_is_never_treated_as_no_repository() -> Checked {
    // hostのmount元が消えたSandboxへの`sbx exec`は、内側のcommandを起動できないまま
    // 終了statusだけを返す。その終了statusは「repositoryが無い」という答えと区別
    // できないため、host側を見ずに`sbx exec`だけへ頼ると同じ結論に落ちてしまう。
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let host = FakeSbx::listing("[]");

    let error = inspect_with(&host, &project, Unmanaged::Refused, &fixture.workspace_root)
        .refused_because("a workspace that cannot be confirmed present is never read as empty")?;
    assert_eq!(error.first_id(), Some(ErrorId::SandboxWorkspaceMissing));
    assert!(
        host.calls().is_empty(),
        "no exec into the sandbox is trusted before the workspace is confirmed present"
    );
    Ok(())
}

#[test]
fn a_workspace_directory_that_cannot_be_observed_is_never_treated_as_no_repository() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    fixture.workspace_is_unobservable(&project)?;
    let host = FakeSbx::listing("[]");

    let error = inspect_with(&host, &project, Unmanaged::Refused, &fixture.workspace_root)
        .refused_because("an unobservable workspace is never read as empty")?;
    assert_eq!(error.first_id(), Some(ErrorId::ProjectPathUnexpectedType));
    Ok(())
}

#[test]
fn a_bare_repository_check_that_could_not_run_is_never_read_as_no_repository() -> Checked {
    // `BARE_GIT_DIR_PROBE`は、内側のshellが実際に走った場合だけstdoutへ印を書く。
    // stdoutが空のまま終わった場合、終了statusが何であれ、内側のcommandが答えた
    // 「不在」として読まない。
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let name = project.sandbox.as_str();
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let command = format!(
        "exec {name} -- sh -c {BARE_GIT_DIR_PROBE} sh {}",
        layout.bare_git_dir()
    );

    for code in [126, 2] {
        let host = clean_host(&fixture, &project)?.answering(&command, code, "");

        let error = inspect_with(&host, &project, Unmanaged::Refused, &fixture.workspace_root)
            .refused_because(
                "a bare-repository check that did not answer never means there is nothing to lose",
            )?;
        assert_eq!(error.first_id(), Some(ErrorId::SandboxCheckUnobservable));
    }
    Ok(())
}

#[test]
fn a_workspace_that_vanishes_between_the_host_check_and_the_repository_probe_is_never_treated_as_no_repository()
-> Checked {
    // hostのworkspace_existsが真を返した直後、次の`sbx exec`までの間にもmount元は
    // 消えうる。その`sbx exec`は内側のshellを起動できないまま終了status `1`だけを
    // 返し、これは`test -e`が答える「不在」の終了statusと同じ値である。終了statusの
    // 3値化だけでは、この2つを区別できない。stdoutに印が無ければ、「repositoryが
    // 無い」ではなく観測できなかったこととして拒否する。
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let name = project.sandbox.as_str();
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let command = format!(
        "exec {name} -- sh -c {BARE_GIT_DIR_PROBE} sh {}",
        layout.bare_git_dir()
    );
    let host = clean_host(&fixture, &project)?.answering(&command, 1, "");

    let error = inspect_with(&host, &project, Unmanaged::Refused, &fixture.workspace_root)
        .refused_because(
            "a workspace that disappeared between the host check and the probe is never read as an empty repository",
        )?;
    assert_eq!(error.first_id(), Some(ErrorId::SandboxCheckUnobservable));
    Ok(())
}
