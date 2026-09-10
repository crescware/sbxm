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
        &project()?,
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
        &project()?,
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
        &project()?,
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
        &project()?,
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
            &project()?,
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
            &project()?,
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
        &project()?,
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
            &project()?,
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
