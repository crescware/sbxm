use crate::diagnostics::ErrorId;

use crate::testing::outcome::{Checked, Refused, Required};

use super::*;
use crate::design::{Fact, SilentProgress};
use crate::testing::provisioning::World;
use crate::testing::repository::*;

#[test]
fn a_silent_refresh_can_disable_opportunistic_tags() -> Checked {
    let host = healthy_clone()?;
    refresh_origin(
        &host,
        "sbxm-example",
        &layout()?.bare_git_dir(),
        TagFollowing::Disabled,
        None,
    )
    .required_because("the silent refresh completes")?;
    assert!(host.ran("fetch --prune --no-tags origin"));
    Ok(())
}

#[test]
fn a_missing_repository_is_cloned_bare_over_https_and_then_verified() -> Checked {
    let host = healthy_clone()?;
    ensure_bare_clone(
        &host,
        "sbxm-example",
        &SandboxOrigin::Github(project()?),
        &layout()?,
        &mut SilentProgress,
    )
    .required_because("clone")?;

    assert!(
        host.ran("git init --bare /home/agent/work/example-repo/.git"),
        "{:?}",
        host.calls()
    );
    assert!(host.ran("remote add origin https://github.com/Example-Org/Example-Repo.git"));
    assert!(host.ran(&format!("config remote.origin.fetch {FETCH_REFSPEC}")));
    assert!(host.ran("fetch --prune --progress origin"));
    assert!(
        host.ran("mkdir -p /home/agent/work/example-repo"),
        "the bare repository lives below the work directory"
    );
    Ok(())
}

#[test]
fn an_existing_repository_of_the_same_project_is_reused() -> Checked {
    let git_dir = layout()?.bare_git_dir();
    let host = healthy_clone()?.holding(&[&git_dir]);
    ensure_bare_clone(
        &host,
        "sbxm-example",
        &SandboxOrigin::Github(project()?),
        &layout()?,
        &mut SilentProgress,
    )
    .required_because("reuse")?;

    assert!(
        !host.ran("git clone"),
        "an existing repository is not recloned"
    );
    assert!(host.ran("fetch --prune --progress origin"));
    Ok(())
}

#[test]
fn an_empty_repository_left_after_git_init_gets_its_missing_origin() -> Checked {
    let world = World::new();
    let git_dir = layout()?.bare_git_dir();
    world.present.borrow_mut().insert(git_dir.clone());
    *world.bare_git_dir.borrow_mut() = Some(git_dir);

    ensure_bare_clone(
        &world,
        "sbxm-example",
        &SandboxOrigin::Github(project()?),
        &layout()?,
        &mut SilentProgress,
    )
    .required_because("resume the empty repository initialization")?;

    assert_eq!(
        world.repository.borrow().get("remote.origin.url").cloned(),
        Some("https://github.com/Example-Org/Example-Repo.git".to_string())
    );
    assert!(world.ran("count-objects -v"));
    Ok(())
}

#[test]
fn a_repository_with_a_matching_origin_but_no_fetch_refspec_gets_it_completed() -> Checked {
    // `remote add origin`の直後、`config remote.origin.fetch`より前に中断した状態を
    // 再現する。originは既に対象repositoryを指しているため、それを作り直さず、
    // 欠けているfetch refspecだけを補う。
    let world = World::new();
    let git_dir = layout()?.bare_git_dir();
    world.present.borrow_mut().insert(git_dir.clone());
    *world.bare_git_dir.borrow_mut() = Some(git_dir);
    world.repository.borrow_mut().insert(
        "remote.origin.url".to_string(),
        "https://github.com/Example-Org/Example-Repo.git".to_string(),
    );

    ensure_bare_clone(
        &world,
        "sbxm-example",
        &SandboxOrigin::Github(project()?),
        &layout()?,
        &mut SilentProgress,
    )
    .required_because("resume the interrupted origin setup")?;

    assert_eq!(
        world
            .repository
            .borrow()
            .get("remote.origin.fetch")
            .cloned(),
        Some(FETCH_REFSPEC.to_string())
    );
    assert!(
        !world.ran("remote add origin"),
        "the already-declared origin is not replaced: {:?}",
        world.invocations()
    );
    Ok(())
}

#[test]
fn a_repository_that_does_not_match_is_refused_instead_of_being_replaced() -> Checked {
    let git_dir = layout()?.bare_git_dir();

    let cases = [
        healthy_clone()?.answering(
            &format!("git --git-dir {git_dir} rev-parse --is-bare-repository"),
            "false\n",
        ),
        healthy_clone()?.answering(
            &format!("git --git-dir {git_dir} config --get-all remote.origin.url"),
            "https://github.com/other-org/other-repo.git\n",
        ),
        healthy_clone()?.answering(
            &format!("git --git-dir {git_dir} config --get-all remote.origin.fetch"),
            "+refs/heads/main:refs/remotes/origin/main\n",
        ),
        healthy_clone()?.failing(&format!("git --git-dir {git_dir} fsck --connectivity-only")),
    ];

    for host in cases {
        let host = host.holding(&[&git_dir]);
        let error = ensure_bare_clone(
            &host,
            "sbxm-example",
            &SandboxOrigin::Github(project()?),
            &layout()?,
            &mut SilentProgress,
        )
        .refused_because("a repository that cannot be proven is refused")?;
        assert_eq!(error.first_id(), Some(ErrorId::SandboxRepositoryUnusable));
        assert!(!host.ran("rm "), "nothing is deleted: {:?}", host.calls());
        assert!(
            !host.ran("remote add origin"),
            "a foreign repository is not completed: {:?}",
            host.calls()
        );
    }
    Ok(())
}

#[test]
fn an_origin_that_is_not_exactly_one_url_is_refused_with_the_number_that_was_found() -> Checked {
    // remoteが1つも無い場合と2つある場合を、同じ「決められない」として扱う。どちらを
    // 採るかを推測すると、宣言と違うrepositoryへfetchしうる。
    let git_dir = layout()?.bare_git_dir();
    let cases = [
        ("", "0"),
        (
            "https://github.com/Example-Org/Example-Repo.git\nhttps://github.com/Other-Org/Other-Repo.git\n",
            "2",
        ),
    ];

    for (answer, count) in cases {
        let host = healthy_clone()?
            .answering(
                &format!("git --git-dir {git_dir} config --get-all remote.origin.url"),
                answer,
            )
            .holding(&[&git_dir]);
        let error = ensure_bare_clone(
            &host,
            "sbxm-example",
            &SandboxOrigin::Github(project()?),
            &layout()?,
            &mut SilentProgress,
        )
        .refused_because("an origin that is not one URL cannot be proven")?;

        let diagnostic = error
            .diagnostics()
            .first()
            .required_because("the refusal carries a diagnostic")?;
        assert_eq!(diagnostic.id, ErrorId::SandboxRepositoryUnusable);
        assert!(
            diagnostic.facts.iter().any(|fact| matches!(
                fact,
                Fact::Translated { value, .. }
                    if value.id == "cause-origin-ambiguous"
                        && value.args.contains(&("count", count.to_string()))
            )),
            "the number of origins that were found is named: {:?}",
            diagnostic.facts
        );
    }
    Ok(())
}

#[test]
fn an_origin_that_is_not_a_github_repository_is_quoted_as_it_stands() -> Checked {
    // canonical IDへ寄せられないremoteは、宣言と比べようがない。読めなかった値を
    // そのまま示さないと、利用者はどのremoteの話か分からない。
    let git_dir = layout()?.bare_git_dir();
    let observed = "https://gitlab.example.com/Example-Org/Example-Repo.git";
    let host = healthy_clone()?
        .answering(
            &format!("git --git-dir {git_dir} config --get-all remote.origin.url"),
            &format!("{observed}\n"),
        )
        .holding(&[&git_dir]);

    let error = ensure_bare_clone(
        &host,
        "sbxm-example",
        &SandboxOrigin::Github(project()?),
        &layout()?,
        &mut SilentProgress,
    )
    .refused_because("a remote that is not on GitHub cannot be this project's")?;

    let diagnostic = error
        .diagnostics()
        .first()
        .required_because("the refusal carries a diagnostic")?;
    assert_eq!(diagnostic.id, ErrorId::SandboxRepositoryUnusable);
    assert!(
        diagnostic.facts.iter().any(|fact| matches!(
            fact,
            Fact::Translated { value, .. }
                if value.id == "cause-origin-not-a-github-repository"
                    && value.args.contains(&("observed", observed.to_string()))
        )),
        "the URL that could not be read is quoted: {:?}",
        diagnostic.facts
    );
    Ok(())
}

#[test]
fn a_step_the_host_could_not_run_is_not_read_as_a_repository_that_must_be_replaced() -> Checked {
    // 検査に答えが返らなかったことを不一致として扱うと、無事なbare repositoryを
    // 作り直せと告げることになる。observationが無いことは不一致ではない。
    let git_dir = layout()?.bare_git_dir();
    let steps = [
        format!(
            "git --git-dir {git_dir} remote add origin https://github.com/Example-Org/Example-Repo.git"
        ),
        format!("git --git-dir {git_dir} config remote.origin.fetch {FETCH_REFSPEC}"),
        format!("git --git-dir {git_dir} rev-parse --is-bare-repository"),
        format!("git --git-dir {git_dir} config --get-all remote.origin.url"),
        format!("git --git-dir {git_dir} config --get-all remote.origin.fetch"),
        format!("git --git-dir {git_dir} fsck --connectivity-only"),
        format!("git --git-dir {git_dir} fetch --prune --progress origin"),
    ];

    for step in steps {
        let host = healthy_clone()?.timing_out(&step);
        let error = ensure_bare_clone(
            &host,
            "sbxm-example",
            &SandboxOrigin::Github(project()?),
            &layout()?,
            &mut SilentProgress,
        )
        .refused_because("a step that did not run stops the preparation")?;
        assert_eq!(
            error.first_id(),
            Some(ErrorId::ExternalCommandTimeout),
            "{step} was reported as something other than the host failure it was"
        );
    }
    Ok(())
}

fn local_metadata() -> Checked<crate::metadata::ProjectMetadata> {
    Ok(crate::metadata::ProjectMetadata {
        repository: crate::repository::RepositoryIdentity::local("/home/user/code/app/.git", "app")
            .required_because("a local repository")?,
        ..crate::testing::metadata::attached("example-org", "example-repo")?
    })
}

#[test]
fn a_github_repository_is_fetched_over_https_from_inside_the_sandbox() -> Checked {
    let metadata = crate::testing::metadata::attached("Example-Org", "Example-Repo")?;
    let origin = SandboxOrigin::of(&metadata).required()?;

    assert_eq!(
        origin.url(),
        "https://github.com/Example-Org/Example-Repo.git"
    );
    origin
        .verify("git@github.com:example-org/example-repo.git")
        .required()?;
    let elsewhere = origin
        .verify("https://github.com/other/repo.git")
        .err()
        .required_because("another repository")?;
    assert_eq!(elsewhere.id, "cause-origin-elsewhere");
    let unknown = origin
        .verify("/srv/repo.git")
        .err()
        .required_because("not a GitHub repository")?;
    assert_eq!(unknown.id, "cause-origin-not-a-github-repository");

    // GitHubへはSandboxから取りに行く。hostのgitは何も書き込まない。
    let host = crate::testing::host::FakeSbx::listing("");
    origin
        .refresh(
            &host,
            "sbxm-example",
            "/home/agent/work/example-repo/.git",
            None,
        )
        .required()?;
    assert!(
        host.calls()
            .iter()
            .all(|call| call.first().is_some_and(|arg| arg == "exec")),
        "{:?}",
        host.calls()
    );
    assert!(host.ran("fetch --prune origin"), "{:?}", host.calls());
    Ok(())
}

#[test]
fn a_host_repository_is_named_in_the_sandbox_without_its_host_path() -> Checked {
    // Sandboxの中から届くremoteは無い。hostの実pathもSandboxへ書かない。
    let origin = SandboxOrigin::of(&local_metadata()?).required()?;

    assert_eq!(origin.url(), "sbxm-host::local/app");
    assert_eq!(
        origin,
        SandboxOrigin::Host {
            repository: std::path::PathBuf::from("/home/user/code/app/.git"),
            project: "local/app".to_string(),
        }
    );
    origin.verify("sbxm-host::local/app").required()?;
    let elsewhere = origin
        .verify("/home/agent/work/app/.git/sbxm/origin.bundle")
        .err()
        .required_because("an origin of an earlier build")?;
    assert_eq!(elsewhere.id, "cause-origin-elsewhere");
    Ok(())
}

/// hostのrepositoryと、Sandboxの中を模したbare repository。
struct Pushing {
    _root: tempfile::TempDir,
    host: std::path::PathBuf,
    sandbox: String,
}

impl Pushing {
    fn new() -> Checked<Pushing> {
        let root = tempfile::tempdir().required()?;
        let host = root.path().join("host");
        std::fs::create_dir(&host).required()?;
        git_in(&host, &["init", "--quiet"])?;
        git_in(
            &host,
            &["commit", "--quiet", "--allow-empty", "-m", "first"],
        )?;
        git_in(&host, &["branch", "--quiet", "topic"])?;
        git_in(&host, &["tag", "v1"])?;
        git_in(root.path(), &["init", "--quiet", "--bare", "sandbox.git"])?;
        let sandbox = root
            .path()
            .join("sandbox.git")
            .to_string_lossy()
            .into_owned();
        Ok(Pushing {
            _root: root,
            host,
            sandbox,
        })
    }

    fn push(&self) -> crate::diagnostics::Result<Vec<PushRefusal>> {
        push_to_sandbox(
            &crate::testing::sandbox::LocalSandbox,
            &self.host,
            "sbxm-example",
            &self.sandbox,
        )
    }

    fn sandbox_refs(&self) -> Checked<String> {
        git_in(
            std::path::Path::new("/"),
            &[
                "--git-dir",
                &self.sandbox,
                "for-each-ref",
                "--format=%(refname)",
            ],
        )
    }
}

#[test]
fn the_host_reaches_the_sandbox_origin_the_way_a_fetch_would() -> Checked {
    // Sandboxの中で`git fetch --prune origin`をしたときと同じものを、hostから書き込む。
    let pushing = Pushing::new()?;
    assert!(pushing.push().required()?.is_empty());
    let refs = pushing.sandbox_refs()?;
    for expected in [
        "refs/remotes/origin/main",
        "refs/remotes/origin/topic",
        "refs/tags/v1",
    ] {
        assert!(
            refs.lines().any(|line| line == expected),
            "{expected}: {refs}"
        );
    }
    // branchは運ばない。worktreeが同じ名前で作るものと衝突する。
    assert!(
        !refs.lines().any(|line| line.starts_with("refs/heads/")),
        "{refs}"
    );

    // hostで消したbranchはSandboxのoriginから消える。Sandboxで作ったtagは残る。
    git_in(&pushing.host, &["branch", "--quiet", "-D", "topic"])?;
    git_in(
        std::path::Path::new("/"),
        &[
            "--git-dir",
            &pushing.sandbox,
            "tag",
            "own",
            "refs/remotes/origin/main",
        ],
    )?;
    assert!(pushing.push().required()?.is_empty());
    let refs = pushing.sandbox_refs()?;
    assert!(
        !refs.lines().any(|line| line == "refs/remotes/origin/topic"),
        "{refs}"
    );
    assert!(refs.lines().any(|line| line == "refs/tags/own"), "{refs}");
    Ok(())
}

#[test]
fn a_symbolic_ref_in_the_sandbox_origin_is_left_with_the_branch_it_names() -> Checked {
    // `git push --prune`は`origin/HEAD`を消そうとし、受け取るgitはsymrefを辿って
    // `origin/main`まで消す。`git fetch --prune`と同じく、symrefは残す。
    let pushing = Pushing::new()?;
    pushing.push().required()?;
    git_in(
        std::path::Path::new("/"),
        &[
            "--git-dir",
            &pushing.sandbox,
            "symbolic-ref",
            "refs/remotes/origin/HEAD",
            "refs/remotes/origin/main",
        ],
    )?;
    git_in(&pushing.host, &["branch", "--quiet", "-D", "topic"])?;

    assert!(pushing.push().required()?.is_empty());

    let refs = pushing.sandbox_refs()?;
    for kept in ["refs/remotes/origin/HEAD", "refs/remotes/origin/main"] {
        assert!(refs.lines().any(|line| line == kept), "{kept}: {refs}");
    }
    assert!(
        !refs.lines().any(|line| line == "refs/remotes/origin/topic"),
        "{refs}"
    );
    Ok(())
}

#[test]
fn a_tag_the_sandbox_already_has_elsewhere_is_left_and_reported() -> Checked {
    let pushing = Pushing::new()?;
    pushing.push().required()?;
    git_in(
        &pushing.host,
        &["commit", "--quiet", "--allow-empty", "-m", "second"],
    )?;
    git_in(&pushing.host, &["tag", "--force", "v1"])?;

    let refused = pushing.push().required()?;

    assert_eq!(
        refused,
        vec![PushRefusal {
            reference: "refs/tags/v1".to_string(),
            reason: "already exists".to_string(),
        }]
    );
    Ok(())
}

#[test]
fn a_sandbox_the_host_cannot_write_to_is_named() -> Checked {
    // sshで届かなければ、gitはrefごとの答えを持たずに終わる。Sandboxのoriginは`sbx exec`で
    // 読めても、書き込めなかったことを名指しする。
    let git_dir = "/home/agent/work/example-repo/.git";
    let push = format!(
        "-c {} push --porcelain --no-verify {} +refs/heads/*:refs/remotes/origin/*",
        sandbox_ssh_config(),
        sandbox_remote("sbxm-example", git_dir)
    );
    let host = crate::testing::host::FakeSbx::listing("").answering(&push, 128, "");

    let error = push_to_sandbox(
        &host,
        std::path::Path::new("/home/user/code/app/.git"),
        "sbxm-example",
        git_dir,
    )
    .refused_because("the host cannot reach the sandbox repository")?;

    assert_eq!(
        error.first_id(),
        Some(crate::diagnostics::ErrorId::SandboxRepositoryUnwritable)
    );
    Ok(())
}

#[test]
fn a_tag_the_sandbox_keeps_is_a_warning_and_a_refused_branch_stops_the_build() -> Checked {
    // tagは上書きしないため、Sandboxの側を残して示す。branchは強制して書き込むため、
    // 断られたら古い`origin/*`のままworktreeを作らない。
    let tag = PushRefusal {
        reference: "refs/tags/v1".to_string(),
        reason: "already exists".to_string(),
    };
    let branch = PushRefusal {
        reference: "refs/remotes/origin/main".to_string(),
        reason: "pre-receive hook declined".to_string(),
    };

    let mut quiet = crate::testing::recorded_output::RecordedOutput::new();
    settle_refusals("sbxm-example", &[], &mut quiet).required()?;
    assert!(quiet.warnings.is_empty());

    let mut warned = crate::testing::recorded_output::RecordedOutput::new();
    settle_refusals("sbxm-example", std::slice::from_ref(&tag), &mut warned).required()?;
    let [warning] = warned.warnings.as_slice() else {
        return Err(crate::testing::outcome::Unmet::new(format!(
            "one warning: {:?}",
            warned.warnings
        )));
    };
    assert_eq!(warning.description.id, "warning-sandbox-tags-kept");
    assert!(warning.facts.contains(&Fact::reference("refs/tags/v1")));

    let error = settle_refusals(
        "sbxm-example",
        &[tag, branch],
        &mut crate::testing::recorded_output::RecordedOutput::new(),
    )
    .refused_because("a branch the sandbox refused is not left behind")?;
    let diagnostic = error
        .diagnostics()
        .first()
        .required_because("the refusal carries a diagnostic")?;
    assert_eq!(diagnostic.id, ErrorId::SandboxRepositoryUnwritable);
    assert!(
        diagnostic
            .facts
            .contains(&Fact::reference("refs/remotes/origin/main")),
        "{:?}",
        diagnostic.facts
    );
    assert!(
        !diagnostic.facts.contains(&Fact::reference("refs/tags/v1")),
        "{:?}",
        diagnostic.facts
    );
    Ok(())
}

#[test]
fn a_reason_git_reports_is_read_whole_even_when_it_holds_parentheses() {
    for (summary, reason) in [
        ("[rejected] (non-fast-forward)", "non-fast-forward"),
        ("[rejected] (already exists)", "already exists"),
        (
            "[remote rejected] (pre-receive hook declined)",
            "pre-receive hook declined",
        ),
        ("[remote rejected] (foo (bar))", "foo (bar)"),
        ("[remote rejected] (see [docs] (here))", "see [docs] (here)"),
        ("odd", "odd"),
    ] {
        assert_eq!(refusal_reason(summary), reason, "{summary}");
    }
}

#[test]
fn a_reason_the_sandbox_wrote_reaches_the_host_without_its_control_characters() -> Checked {
    // `[remote rejected]`の理由は、Sandboxのgitやhookが決める。hostの端末を操作する
    // 文字は、見える形にしてから渡す。
    let git_dir = "/home/agent/work/example-repo/.git";
    let push = format!(
        "-c {} push --porcelain --no-verify {} refs/tags/*:refs/tags/*",
        sandbox_ssh_config(),
        sandbox_remote("sbxm-example", git_dir)
    );
    let host = crate::testing::host::FakeSbx::listing("").answering(
        &push,
        1,
        "To x\n!\trefs/tags/v1:refs/tags/v1\t[remote rejected] (hook \u{1b}]0;x\u{7} said (no))\nDone\n",
    );

    let refused = push_to_sandbox(
        &host,
        std::path::Path::new("/home/user/code/app/.git"),
        "sbxm-example",
        git_dir,
    )
    .required()?;

    assert_eq!(
        refused,
        vec![PushRefusal {
            reference: "refs/tags/v1".to_string(),
            reason: "hook \\u{1b}]0;x\\u{7} said (no)".to_string(),
        }]
    );
    Ok(())
}

#[test]
fn a_sandbox_built_with_an_earlier_origin_is_pointed_at_a_rebuild() -> Checked {
    // 以前のsbxmは、hostから送ったbundleをoriginにしていた。作り直せば揃うことを示す。
    let git_dir = layout()?.bare_git_dir();
    let host = healthy_clone()?.answering(
        &format!("git --git-dir {git_dir} config --get-all remote.origin.url"),
        &format!("{git_dir}/sbxm/origin.bundle\n"),
    );
    let origin = SandboxOrigin::Host {
        repository: std::path::PathBuf::from("/home/user/code/app/.git"),
        project: "local/app".to_string(),
    };

    let error = verify_bare_clone(&host, "sbxm-example", &origin, &git_dir)
        .refused_because("the origin is not the one the host writes")?;

    let diagnostic = error
        .diagnostics()
        .first()
        .required_because("the refusal carries a diagnostic")?;
    assert_eq!(diagnostic.id, ErrorId::SandboxRepositoryUnusable);
    let remediation = diagnostic
        .remediation
        .as_ref()
        .required_because("the refusal says what to do")?;
    assert!(
        remediation
            .commands
            .iter()
            .any(|command| command.as_str() == "sbxm rebuild local/app"),
        "{remediation:?}"
    );
    Ok(())
}

#[test]
fn a_host_repository_without_commits_sends_nothing() -> Checked {
    let root = tempfile::tempdir().required()?;
    let host = root.path().join("empty");
    std::fs::create_dir(&host).required()?;
    git_in(&host, &["init", "--quiet"])?;
    let origin = SandboxOrigin::Host {
        repository: host,
        project: "local/empty".to_string(),
    };
    let recorded = crate::testing::host::FakeSbx::listing("");

    let error = origin
        .refresh(
            &recorded,
            "sbxm-example",
            "/home/agent/work/empty/.git",
            None,
        )
        .refused_because("there is nothing to send")?;

    assert_eq!(
        error.first_id(),
        Some(crate::diagnostics::ErrorId::HostRepositoryEmpty)
    );
    assert!(!recorded.ran("push"), "{:?}", recorded.calls());
    Ok(())
}

#[test]
fn a_host_repository_that_is_gone_does_not_lead_git_to_the_one_around_it() -> Checked {
    // 利用者の`$HOME`がrepositoryであることは珍しくない。案件のrepositoryが無くなった
    // とき、その外側のrepositoryへsbxmのrefを書き込まない。
    let root = tempfile::tempdir().required()?;
    git_in(root.path(), &["init", "--quiet"])?;
    let gone = root.path().join("project");
    std::fs::create_dir(&gone).required()?;

    let outcome = host_git(
        &crate::boundary::host::RealHost,
        &gone,
        &["rev-parse", "--show-toplevel"],
        None,
        crate::boundary::host::TimeoutClass::LocalFilesystem,
    )
    .required()?;
    assert!(!outcome.success(), "{}", outcome.stdout_text());
    Ok(())
}

#[test]
fn a_host_repository_that_was_moved_away_is_named_as_missing() -> Checked {
    // 無いのはgitではなく、登録したrepositoryである。
    let root = tempfile::tempdir().required()?;
    let error = host_git(
        &crate::boundary::host::RealHost,
        &root.path().join("moved-away"),
        &["rev-parse", "--show-toplevel"],
        None,
        crate::boundary::host::TimeoutClass::LocalFilesystem,
    )
    .refused_because("the repository is gone")?;
    assert_eq!(error.first_id(), Some(ErrorId::HostRepositoryMissing));
    Ok(())
}
