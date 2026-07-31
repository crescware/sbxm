use crate::testing::outcome::{Checked, Refused, Required};

use super::*;
use crate::testing::repository::*;
use crate::ui::SilentProgress;

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
    assert!(host.ran("fetch --prune origin"));
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
    assert!(host.ran("fetch --prune origin"));
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
    }
    Ok(())
}
