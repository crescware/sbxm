use super::*;
use crate::testing::host::FakeSbx;
use crate::testing::project::{Registered, fixture};
use crate::testing::protection::clean_host;
use crate::testing::value::COMMIT;

fn inspect_with(host: &FakeSbx, project: &Registered, unmanaged: Unmanaged) -> Result<Protection> {
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    inspect(
        host,
        project.sandbox.as_str(),
        &layout,
        &project.metadata,
        unmanaged,
    )
}

#[test]
fn a_clean_managed_worktree_passes_and_is_reported() {
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    let host = clean_host(&fixture, &project);

    let protection =
        inspect_with(&host, &project, Unmanaged::Refused).expect("a clean worktree passes");
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
}

#[test]
fn work_that_is_not_committed_or_not_pushed_stops_the_run() {
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let name = project.sandbox.as_str();
    let managed = format!("{}/example-repo.tree-0", layout.bare_root());

    let dirty = clean_host(&fixture, &project).answering(
        &format!("exec {name} -- git -C {managed} status --porcelain=v2 -z --untracked-files=all"),
        0,
        "? untracked.txt\0",
    );
    let unpushed = clean_host(&fixture, &project).answering(
        &format!("exec {name} -- git -C {managed} rev-list --count origin/main..HEAD"),
        0,
        "2\n",
    );
    let no_upstream = clean_host(&fixture, &project).answering(
            &format!(
                "exec {name} -- git -C {managed} rev-parse --abbrev-ref --symbolic-full-name @{{upstream}}"
            ),
            1,
            "",
        );
    let in_progress = clean_host(&fixture, &project).answering(
        &format!("exec {name} -- test -e {managed}/.git/MERGE_HEAD"),
        0,
        "",
    );

    for host in [dirty, unpushed, no_upstream, in_progress] {
        let error = inspect_with(&host, &project, Unmanaged::Refused)
            .expect_err("unsaved work is never destroyed");
        assert_eq!(error.first_id(), Some(ErrorId::UnsavedWork));
    }
}

#[test]
fn a_check_that_could_not_run_is_never_read_as_a_pass() {
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let name = project.sandbox.as_str();
    let managed = format!("{}/example-repo.tree-0", layout.bare_root());

    // `sbx exec`が内側のcommandを起動できなかったことを示す終了status。
    let marker = clean_host(&fixture, &project).answering(
        &format!("exec {name} -- test -e {managed}/.git/MERGE_HEAD"),
        126,
        "",
    );
    let head = clean_host(&fixture, &project).answering(
        &format!("exec {name} -- git -C {managed} symbolic-ref --quiet --short HEAD"),
        127,
        "",
    );
    let upstream = clean_host(&fixture, &project).answering(
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
        let error = inspect_with(&host, &project, Unmanaged::Allowed)
            .expect_err("a check that did not answer never means the worktree is safe");
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
            .expect("the runtime's own failure is kept with the diagnostic");
        assert_eq!(external.program, "sbx");
        assert_eq!(external.exit_status, format!("exit status: {code}"));
    }
}

#[test]
fn an_unmanaged_worktree_is_refused_for_rebuild_and_examined_for_destroy() {
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let name = project.sandbox.as_str();
    let managed = format!("{}/example-repo.tree-0", layout.bare_root());
    let extra = format!("{}/agent-scratch", layout.bare_root());

    let host = clean_host(&fixture, &project)
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

    let error = inspect_with(&host, &project, Unmanaged::Refused)
        .expect_err("rebuild cannot recreate a worktree it does not know about");
    assert_eq!(error.first_id(), Some(ErrorId::UnmanagedWorktreePresent));

    let protection = inspect_with(&host, &project, Unmanaged::Allowed)
        .expect("destroy examines it under the same rules");
    assert_eq!(protection.worktrees.len(), 2);
    assert_eq!(protection.worktrees[1].kind, Kind::Unmanaged);
    assert_eq!(protection.worktrees[1].remote, Remote::Reachable);
}

#[test]
fn a_worktree_that_is_not_an_artifact_of_this_project_is_not_reported_as_unsaved_work() {
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let name = project.sandbox.as_str();
    let listing = format!(
        "exec {name} -- git --git-dir {} worktree list --porcelain -z",
        layout.bare_git_dir()
    );

    // bare rootの外を指すworktree。
    let outside = clean_host(&fixture, &project).answering(
        &listing,
        0,
        &format!(
            "worktree {}\0bare\0\0worktree /home/agent/elsewhere\0branch refs/heads/main\0\0",
            layout.bare_root()
        ),
    );
    let error = inspect_with(&outside, &project, Unmanaged::Allowed)
        .expect_err("a path outside the repository is a security refusal");
    assert_eq!(error.first_id(), Some(ErrorId::WorktreeOutsideRepository));
}

#[test]
fn a_sandbox_whose_git_lists_no_worktree_has_nothing_to_lose() {
    // 構築や再構築が途中で終わったSandboxには、checkoutされた作業が存在しない。
    // 宣言との食い違いを理由に止めると、作り直す手段がなくなる。
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let name = project.sandbox.as_str();
    let listing = format!(
        "exec {name} -- git --git-dir {} worktree list --porcelain -z",
        layout.bare_git_dir()
    );

    let empty = clean_host(&fixture, &project).answering(
        &listing,
        0,
        &format!("worktree {}\0bare\0\0", layout.bare_root()),
    );
    let protection = inspect_with(&empty, &project, Unmanaged::Refused)
        .expect("a sandbox holding no worktree can be replaced");
    assert!(protection.worktrees.is_empty());
}

#[test]
fn a_detached_head_that_no_remote_reaches_stops_the_run() {
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let name = project.sandbox.as_str();
    let managed = format!("{}/example-repo.tree-0", layout.bare_root());

    let host = clean_host(&fixture, &project)
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

    let error = inspect_with(&host, &project, Unmanaged::Allowed)
        .expect_err("commits no remote holds are not thrown away");
    assert_eq!(error.first_id(), Some(ErrorId::UnsavedWork));
}

#[test]
fn a_sandbox_without_the_shared_repository_has_nothing_to_lose() {
    // 構築が途中で終わったSandboxには、この案件の作業が1件もない。worktreeが
    // 観測できないことを、失うものがある徴候として読まない。
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    let name = project.sandbox.as_str();
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let host = clean_host(&fixture, &project).answering(
        &format!("exec {name} -- test -e {}", layout.bare_git_dir()),
        1,
        "",
    );

    let protection = inspect_with(&host, &project, Unmanaged::Refused)
        .expect("a sandbox that holds no repository can be replaced");
    assert!(protection.worktrees.is_empty());
}
