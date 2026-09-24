use std::fs;
use std::path::PathBuf;
use std::time::{Duration, UNIX_EPOCH};

use crate::diagnostics::ErrorId;

use crate::testing::outcome::{Checked, Refused, Required};
use crate::testing::repository::git_in;
use crate::testing::sandbox::LocalSandbox;

use super::*;

const NAMESPACE: &str = "sbxm-example-org-example-repo-99a40327a69b";

/// Sandboxの中を模したbare repositoryと、hostのrepository。
struct Repositories {
    _root: tempfile::TempDir,
    /// bare repositoryのgit directory。
    sandbox: PathBuf,
    /// Sandbox内のmanaged worktree。
    worktree: PathBuf,
    host: PathBuf,
    bundles: PathBuf,
}

impl Repositories {
    fn new() -> Checked<Repositories> {
        let root = tempfile::tempdir().required()?;
        let host = root.path().join("host");
        fs::create_dir(&host).required()?;
        git_in(&host, &["init", "--quiet"])?;
        git_in(&host, &["commit", "--quiet", "--allow-empty", "-m", "host"])?;

        let bare = root.path().join("sandbox");
        fs::create_dir(&bare).required()?;
        git_in(&bare, &["init", "--quiet", "--bare", ".git"])?;
        let sandbox = bare.join(".git");
        let worktree = bare.join("example-repo.tree-0");
        let git_dir = sandbox.to_string_lossy().into_owned();
        git_in(
            root.path(),
            &[
                "--git-dir",
                &git_dir,
                "worktree",
                "add",
                "--quiet",
                "-b",
                "main",
                &worktree.to_string_lossy(),
            ],
        )?;
        git_in(
            &worktree,
            &["commit", "--quiet", "--allow-empty", "-m", "first"],
        )?;
        Ok(Repositories {
            bundles: root.path().join("bundles"),
            _root: root,
            sandbox,
            worktree,
            host,
        })
    }

    fn git_dir(&self) -> String {
        self.sandbox.to_string_lossy().into_owned()
    }

    fn commit(&self, message: &str) -> Checked<String> {
        git_in(
            &self.worktree,
            &["commit", "--quiet", "--allow-empty", "-m", message],
        )?;
        git_in(&self.worktree, &["rev-parse", "HEAD"])
    }

    /// Sandboxからbundleを受け取り、hostへ取り込む。
    fn fetch(&self, label: &str) -> Checked<Vec<RefChange>> {
        let received = receive_bundle(
            &LocalSandbox,
            "sbxm-example",
            &self.git_dir(),
            &self.bundles,
            label,
        )
        .required()?
        .required_because("the sandbox has refs to save")?;
        import_bundle(
            &LocalSandbox,
            &self.host,
            &received.path,
            NAMESPACE,
            &received.label,
        )
        .required_because("the bundle is imported")
    }

    fn host_ref(&self, reference: &str) -> Checked<String> {
        git_in(&self.host, &["rev-parse", "--verify", reference])
    }
}

fn reference(kind_and_name: &str) -> String {
    format!("refs/sbx/{NAMESPACE}/{kind_and_name}")
}

#[test]
fn stamps_follow_the_calendar_in_utc() {
    assert_eq!(stamp(UNIX_EPOCH), "19700101T000000Z");
    // 閏日と、百年ごとの例外を跨ぐ日。
    assert_eq!(
        stamp(UNIX_EPOCH + Duration::from_secs(951_782_400)),
        "20000229T000000Z"
    );
    assert_eq!(
        stamp(UNIX_EPOCH + Duration::from_secs(1_790_158_501)),
        "20260923T101501Z"
    );
}

#[test]
fn a_first_fetch_brings_branches_and_worktree_heads_under_the_sbxm_namespace() -> Checked {
    let repositories = Repositories::new()?;
    let tip = git_in(&repositories.worktree, &["rev-parse", "HEAD"])?;
    let host_main = repositories.host_ref("refs/heads/main")?;

    let changes = repositories.fetch("20260923T100000Z")?;

    assert_eq!(
        changes,
        vec![
            RefChange::Created {
                reference: reference("heads/main")
            },
            RefChange::Created {
                reference: reference("worktrees/example-repo.tree-0")
            },
        ]
    );
    assert_eq!(repositories.host_ref(&reference("heads/main"))?, tip);
    // hostのbranchにもtagにも触れない。
    assert_eq!(repositories.host_ref("refs/heads/main")?, host_main);
    assert!(git_in(&repositories.host, &["tag", "--list"])?.is_empty());
    // 一時的な名前空間も、Sandboxの一時refも残さない。
    assert!(git_in(&repositories.host, &["for-each-ref", "refs/sbx-incoming/"])?.is_empty());
    assert!(git_in(&repositories.worktree, &["for-each-ref", "refs/sbxm/save/"])?.is_empty());
    Ok(())
}

#[test]
fn rewritten_and_deleted_refs_keep_their_old_tips_under_archive() -> Checked {
    let repositories = Repositories::new()?;
    repositories.fetch("20260923T100000Z")?;

    // 早送り。
    let advanced = repositories.commit("second")?;
    let changes = repositories.fetch("20260923T100100Z")?;
    assert!(changes.contains(&RefChange::Updated {
        reference: reference("heads/main")
    }));
    assert_eq!(repositories.host_ref(&reference("heads/main"))?, advanced);

    // 早送りでない書き換え。前の先端は退避し、消さない。
    git_in(
        &repositories.worktree,
        &["reset", "--quiet", "--hard", "HEAD~1"],
    )?;
    let rewritten = repositories.commit("rewritten")?;
    let changes = repositories.fetch("20260923T100200Z")?;
    let archived = reference("archive/20260923T100200Z/heads/main");
    assert!(changes.contains(&RefChange::Replaced {
        reference: reference("heads/main"),
        archived: archived.clone(),
    }));
    assert_eq!(repositories.host_ref(&reference("heads/main"))?, rewritten);
    assert_eq!(repositories.host_ref(&archived)?, advanced);

    // Sandboxで消えたbranch。
    git_in(&repositories.worktree, &["branch", "--quiet", "topic"])?;
    repositories.fetch("20260923T100300Z")?;
    let topic = repositories.host_ref(&reference("heads/topic"))?;
    git_in(
        &repositories.worktree,
        &["branch", "--quiet", "-D", "topic"],
    )?;
    let changes = repositories.fetch("20260923T100400Z")?;
    let archived = reference("archive/20260923T100400Z/heads/topic");
    assert!(changes.contains(&RefChange::Deleted {
        reference: reference("heads/topic"),
        archived: archived.clone(),
    }));
    assert!(repositories.host_ref(&reference("heads/topic")).is_err());
    assert_eq!(repositories.host_ref(&archived)?, topic);

    // 退避したrefは、後の取り込みでも消えない。
    repositories.fetch("20260923T100500Z")?;
    assert_eq!(repositories.host_ref(&archived)?, topic);
    Ok(())
}

#[test]
fn a_fetch_with_nothing_new_changes_nothing() -> Checked {
    let repositories = Repositories::new()?;
    repositories.fetch("20260923T100000Z")?;
    assert!(repositories.fetch("20260923T100100Z")?.is_empty());
    Ok(())
}

#[test]
fn a_repository_without_refs_has_nothing_to_save() -> Checked {
    let root = tempfile::tempdir().required()?;
    git_in(root.path(), &["init", "--quiet", "--bare", "empty.git"])?;
    let git_dir = root.path().join("empty.git").to_string_lossy().into_owned();
    let bundles = root.path().join("bundles");

    let received =
        receive_bundle(&LocalSandbox, "sbxm-example", &git_dir, &bundles, "stamp").required()?;
    assert!(received.is_none());
    assert!(
        fs::read_dir(&bundles).required()?.next().is_none(),
        "no empty bundle is kept"
    );
    Ok(())
}

#[test]
fn a_bundle_received_in_the_same_second_gets_its_own_name() -> Checked {
    let repositories = Repositories::new()?;
    let first = receive_bundle(
        &LocalSandbox,
        "sbxm-example",
        &repositories.git_dir(),
        &repositories.bundles,
        "20260923T100000Z",
    )
    .required()?
    .required()?;
    let second = receive_bundle(
        &LocalSandbox,
        "sbxm-example",
        &repositories.git_dir(),
        &repositories.bundles,
        "20260923T100000Z",
    )
    .required()?
    .required()?;
    assert_eq!(first.label, "20260923T100000Z");
    assert_eq!(second.label, "20260923T100000Z-2");
    assert!(first.path.exists() && second.path.exists());
    Ok(())
}

#[test]
fn a_damaged_bundle_is_refused_before_anything_is_imported() -> Checked {
    let repositories = Repositories::new()?;
    fs::create_dir_all(&repositories.bundles).required()?;
    let damaged = repositories.bundles.join("damaged.bundle");
    fs::write(&damaged, b"# v2 git bundle\nnot really\n").required()?;

    let error = import_bundle(&LocalSandbox, &repositories.host, &damaged, NAMESPACE, "x")
        .refused_because("the bundle does not verify")?;
    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandFailed));
    assert!(git_in(&repositories.host, &["for-each-ref", "refs/sbx"])?.is_empty());
    Ok(())
}

#[test]
fn only_the_newest_bundles_are_kept() -> Checked {
    let root = tempfile::tempdir().required()?;
    for name in ["1.bundle", "2.bundle", "3.bundle", "4.bundle", "notes.txt"] {
        fs::write(root.path().join(name), b"").required()?;
    }
    prune_bundles(root.path(), 2).required()?;
    let mut left: Vec<String> = fs::read_dir(root.path())
        .required()?
        .filter_map(std::result::Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    left.sort();
    assert_eq!(left, vec!["3.bundle", "4.bundle", "notes.txt"]);

    // まだ無いdirectoryには、消すものも無い。
    prune_bundles(&root.path().join("absent"), 2).required()?;
    Ok(())
}
