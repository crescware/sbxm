use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::boundary::host::RealHost;
use crate::diagnostics::ErrorId;
use crate::project::SandboxName;
use crate::testing::host::{FakeSbx, Unrunnable};
use crate::testing::outcome::{Checked, Refused, Required, Unmet};
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

/// 模したhostのrepositoryが持つcommit。
const HELD: &str = "1111111111111111111111111111111111111111";

/// `commit`へ届くhostのrefを列挙する引数。
fn contains(commit: &str) -> String {
    format!(
        "for-each-ref --format=%(refname) --contains={commit} refs/heads/ refs/tags/ refs/sbx/{SANDBOX}/"
    )
}

#[test]
fn a_commit_whose_reaching_refs_cannot_be_listed_is_not_reached_from_nowhere() -> Checked {
    // hostにあるcommitでも、どのrefから届くかを読めなければ、どこからも届かないとは
    // 言えない。hostのrepositoryを読めなかったこととして返す。
    let host = FakeSbx::listing("").answering(&contains(HELD), 128, "");

    let observation = observe_host_origin(
        &host,
        Path::new("/srv/code/app"),
        &sandbox()?,
        &[candidate(HELD, Some("refs/remotes/origin/main"))],
    )
    .required()?;

    assert_eq!(
        observation,
        OriginObservation::Unobservable {
            reason: UnobservableReason::HostRepositoryUnreadable
        }
    );
    assert!(
        host.ran(&format!("cat-file -e {HELD}^{{commit}}")),
        "{:?}",
        host.calls()
    );
    Ok(())
}

#[test]
fn a_question_the_host_git_could_not_answer_is_an_error_not_an_observation() -> Checked {
    // hostのgitが答えなければ、commitの有無もrefの到達も観測していない。後続のcandidate
    // へ進まずに止まる。
    const LATER: &str = "2222222222222222222222222222222222222222";
    let steps = [format!("cat-file -e {HELD}^{{commit}}"), contains(HELD)];

    for step in steps {
        let host = Unrunnable::timing_out(FakeSbx::listing(""), &step);
        let error = observe_host_origin(
            &host,
            Path::new("/srv/code/app"),
            &sandbox()?,
            &[candidate(HELD, None), candidate(LATER, None)],
        )
        .refused_because("an unanswered host git is not an observation")?;
        assert_eq!(
            error.first_id(),
            Some(ErrorId::ExternalCommandTimeout),
            "{step}"
        );
        assert!(!host.inner.ran(LATER), "{step}: {:?}", host.inner.calls());
    }
    Ok(())
}
