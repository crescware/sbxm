use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::boundary::host::RealHost;
use crate::project::SandboxName;
use crate::testing::outcome::{Checked, Required, Unmet};
use crate::testing::repository::git_in;

use super::super::{CommitCandidate, OriginObservation, Reachability, UnobservableReason};
use super::observe_host_origin;

const SANDBOX: &str = "sbxm-local-app-a888dc9878c3";

fn sandbox() -> Checked<SandboxName> {
    let name = SandboxName::derive(
        &crate::project::ProjectId::parse("local/app")
            .required()?
            .canonical(),
    );
    assert_eq!(name.as_str(), SANDBOX);
    Ok(name)
}

/// commitを1つ持つhostのrepository。
fn host_repository(root: &Path) -> Checked<(PathBuf, String)> {
    let path = root.join("app");
    std::fs::create_dir_all(&path).required()?;
    git_in(&path, &["init", "--quiet"])?;
    git_in(
        &path,
        &["commit", "--quiet", "--allow-empty", "-m", "first"],
    )?;
    let head = git_in(&path, &["rev-parse", "HEAD"])?;
    Ok((path, head))
}

fn candidate(commit: &str, upstream: Option<&str>) -> CommitCandidate {
    CommitCandidate::new(
        "HEAD".to_string(),
        commit.to_string(),
        upstream.map(str::to_owned),
    )
}

fn reaching(observation: &OriginObservation, commit: &str) -> Checked<BTreeSet<String>> {
    match observation {
        OriginObservation::Observed { reachable_from, .. } => reachable_from
            .get(commit)
            .cloned()
            .required_because("every candidate is observed"),
        OriginObservation::Unobservable { reason } => {
            Err(Unmet::new(format!("the host was not observed: {reason:?}")))
        }
    }
}

#[test]
fn a_commit_on_a_host_branch_is_pushed_to_that_branch() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let (repository, head) = host_repository(dir.path())?;
    let pushed = candidate(&head, Some("refs/remotes/origin/main"));

    let observation = observe_host_origin(
        &RealHost,
        &repository,
        &sandbox()?,
        std::slice::from_ref(&pushed),
    )
    .required()?;

    assert_eq!(
        reaching(&observation, &head)?,
        BTreeSet::from(["refs/remotes/origin/main".to_string()])
    );
    assert_eq!(
        Reachability::classify(&pushed, &observation),
        Reachability::Pushed {
            upstream: "refs/remotes/origin/main".to_string()
        }
    );
    Ok(())
}

#[test]
fn tags_and_commits_saved_for_this_sandbox_keep_a_commit() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let (repository, head) = host_repository(dir.path())?;
    git_in(
        &repository,
        &["commit", "--quiet", "--allow-empty", "-m", "tagged"],
    )?;
    git_in(&repository, &["tag", "v1"])?;
    let tagged = git_in(&repository, &["rev-parse", "HEAD"])?;
    git_in(
        &repository,
        &["commit", "--quiet", "--allow-empty", "-m", "saved"],
    )?;
    let saved = git_in(&repository, &["rev-parse", "HEAD"])?;
    git_in(
        &repository,
        &[
            "update-ref",
            &format!("refs/sbx/{SANDBOX}/heads/work"),
            &saved,
        ],
    )?;
    git_in(
        &repository,
        &["update-ref", "refs/sbx/sbxm-other/heads/work", &saved],
    )?;
    // hostのmainからはもう辿れない。
    git_in(&repository, &["reset", "--quiet", "--hard", &head])?;
    git_in(&repository, &["tag", "-d", "v1"])?;
    git_in(&repository, &["tag", "v1", &tagged])?;

    let observation = observe_host_origin(
        &RealHost,
        &repository,
        &sandbox()?,
        &[candidate(&tagged, None), candidate(&saved, None)],
    )
    .required()?;

    assert_eq!(
        reaching(&observation, &tagged)?,
        BTreeSet::from([
            "refs/tags/v1".to_string(),
            format!("host:refs/sbx/{SANDBOX}/heads/work"),
        ])
    );
    // 別のSandboxのために保存した先端は、このSandboxの作業を残す根拠にならない。
    assert_eq!(
        reaching(&observation, &saved)?,
        BTreeSet::from([format!("host:refs/sbx/{SANDBOX}/heads/work")])
    );
    let OriginObservation::Observed { tips, .. } = &observation else {
        return Err(Unmet::new("observed"));
    };
    assert_eq!(tips.get("refs/remotes/origin/main"), Some(&head));
    assert!(
        !tips.keys().any(|label| label.contains("sbxm-other")),
        "{tips:?}"
    );
    Ok(())
}

#[test]
fn a_commit_the_host_does_not_have_is_reached_from_nowhere() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let (repository, _) = host_repository(dir.path())?;
    let missing = "0123456789abcdef0123456789abcdef01234567";
    let only_in_sandbox = candidate(missing, Some("refs/remotes/origin/main"));

    let observation = observe_host_origin(
        &RealHost,
        &repository,
        &sandbox()?,
        std::slice::from_ref(&only_in_sandbox),
    )
    .required()?;

    assert_eq!(reaching(&observation, missing)?, BTreeSet::new());
    assert_eq!(
        Reachability::classify(&only_in_sandbox, &observation),
        Reachability::Unreachable
    );
    Ok(())
}

#[test]
fn a_host_repository_that_cannot_be_read_is_not_taken_as_empty() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let missing = dir.path().join("moved-away");
    std::fs::create_dir_all(&missing).required()?;

    let observation = observe_host_origin(
        &RealHost,
        &missing,
        &sandbox()?,
        &[candidate("0123456789abcdef0123456789abcdef01234567", None)],
    )
    .required()?;

    assert_eq!(
        observation,
        OriginObservation::Unobservable {
            reason: UnobservableReason::HostRepositoryUnreadable
        }
    );
    Ok(())
}
