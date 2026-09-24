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

#[test]
fn saving_to_the_host_keeps_the_bundle_in_the_project_and_prunes_old_ones() -> Checked {
    let repositories = Repositories::new()?;
    let parent = tempfile::tempdir().required()?;
    let canonical = crate::project::ProjectId::parse("Example-Org/Example-Repo")
        .required()?
        .canonical();
    let paths = crate::paths::ProjectPaths::at(parent.path(), &canonical);
    let sandbox = crate::project::SandboxName::derive(&canonical);

    for round in 0..=KEPT_BUNDLES {
        repositories.commit(&format!("round {round}"))?;
        let changes = save_to_host(
            &LocalSandbox,
            &paths,
            &sandbox,
            &repositories.git_dir(),
            &repositories.host,
        )
        .required()?
        .required_because("the sandbox has refs to save")?;
        assert!(!changes.is_empty(), "round {round} saved something");
        // 同じ秒に続けて受け取っても、名前は重ならない。
    }
    assert_eq!(
        repositories.host_ref(&format!("refs/sbx/{}/heads/main", sandbox.as_str()))?,
        git_in(&repositories.worktree, &["rev-parse", "HEAD"])?
    );
    let kept = fs::read_dir(paths.bundles_dir()).required()?.count();
    assert_eq!(kept, KEPT_BUNDLES, "only the newest bundles are kept");
    Ok(())
}

#[test]
fn every_tip_saved_on_the_host_is_listed_including_archived_ones() -> Checked {
    let repositories = Repositories::new()?;
    repositories.fetch("20260923T100000Z")?;
    let first = git_in(&repositories.worktree, &["rev-parse", "HEAD"])?;
    git_in(
        &repositories.worktree,
        &[
            "commit",
            "--quiet",
            "--amend",
            "--allow-empty",
            "-m",
            "amended",
        ],
    )?;
    repositories.fetch("20260923T100100Z")?;

    let sandbox = crate::project::SandboxName::derive(
        &crate::project::ProjectId::parse("Example-Org/Example-Repo")
            .required()?
            .canonical(),
    );
    assert_eq!(sandbox.as_str(), NAMESPACE);
    let tips = saved_tips(&LocalSandbox, &repositories.host, &sandbox).required()?;
    assert!(
        tips.iter().any(
            |tip| tip.reference == reference("archive/20260923T100100Z/heads/main")
                && tip.commit == first
        ),
        "{tips:?}"
    );
    assert!(
        tips.iter()
            .any(|tip| tip.reference == reference("heads/main"))
    );

    // 読めないrepositoryは、保存済みの先端を持たないものとして扱う。
    let missing = crate::paths::ProjectPaths::at(
        &repositories.host.join("absent-project"),
        &crate::project::ProjectId::parse("Example-Org/Example-Repo")
            .required()?
            .canonical(),
    );
    let metadata =
        crate::testing::repository::metadata(crate::metadata::CreationMode::Attached, None, 1)?;
    assert!(saved_on_host(&LocalSandbox, &missing, &metadata).is_empty());
    Ok(())
}

/// hostのrepositoryと、Sandboxの中のbundleの置き場所を模したdirectory。
struct Sending {
    root: tempfile::TempDir,
    host: PathBuf,
}

impl Sending {
    fn new() -> Checked<Sending> {
        let root = tempfile::tempdir().required()?;
        let host = root.path().join("host");
        fs::create_dir(&host).required()?;
        git_in(&host, &["init", "--quiet"])?;
        Ok(Sending { root, host })
    }

    fn staging(&self) -> PathBuf {
        self.root.path().join("bundles")
    }

    fn destination(&self) -> PathBuf {
        self.root.path().join("sandbox/.git/sbxm/origin.bundle")
    }

    fn send(
        &self,
        host: &dyn crate::boundary::host::HostEnvironment,
    ) -> crate::diagnostics::Result<()> {
        send_to_sandbox(
            host,
            &self.host,
            &["--branches", "--tags"],
            &self.staging(),
            "sandbox",
            &self.destination().to_string_lossy(),
        )
    }
}

/// directoryに残ったfileの名前。
fn names_in(directory: &std::path::Path) -> Vec<String> {
    fs::read_dir(directory)
        .map(|entries| {
            entries
                .filter_map(std::result::Result::ok)
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn the_host_branches_and_tags_reach_the_sandbox_as_one_bundle() -> Checked {
    let sending = Sending::new()?;
    git_in(
        &sending.host,
        &["commit", "--quiet", "--allow-empty", "-m", "first"],
    )?;
    git_in(&sending.host, &["tag", "v1"])?;
    git_in(&sending.host, &["branch", "feature"])?;

    sending.send(&LocalSandbox).required()?;

    let destination = sending.destination();
    let heads = git_in(
        sending.root.path(),
        &["bundle", "list-heads", &destination.to_string_lossy()],
    )?;
    for reference in ["refs/heads/main", "refs/heads/feature", "refs/tags/v1"] {
        assert!(heads.contains(reference), "{reference}: {heads}");
    }
    // 送るためのbundleも、受け取りかけのfileも残さない。
    assert_eq!(names_in(&sending.staging()), Vec::<String>::new());
    let placed = destination.parent().required()?;
    assert_eq!(names_in(placed), ["origin.bundle"]);
    Ok(())
}

#[test]
fn a_host_repository_without_commits_sends_nothing() -> Checked {
    let sending = Sending::new()?;

    let error = require_something_to_send(&LocalSandbox, &sending.host)
        .refused_because("nothing to send")?;
    assert_eq!(error.first_id(), Some(ErrorId::HostRepositoryEmpty));

    git_in(
        &sending.host,
        &["commit", "--quiet", "--allow-empty", "-m", "first"],
    )?;
    require_something_to_send(&LocalSandbox, &sending.host).required()?;
    Ok(())
}

#[test]
fn a_bundle_that_does_not_arrive_whole_replaces_nothing() -> Checked {
    // Sandboxの中の手順は、受け取った中身のdigestが違えば何も置かない。
    let sending = Sending::new()?;
    let destination = sending.destination();
    let parent = destination.parent().required()?;
    fs::create_dir_all(parent).required()?;
    fs::write(&destination, "previous\n").required()?;
    let input = sending.root.path().join("input");
    fs::write(&input, "partial").required()?;

    let spec = crate::boundary::host::CommandSpec::capture(
        "sh",
        &[
            "-c",
            PLACE_BUNDLE,
            "sh",
            &destination.to_string_lossy(),
            &crate::hash::sha256_hex(b"whole"),
        ],
    )
    .with_input_file(&input);
    let outcome = crate::boundary::host::HostEnvironment::run(&LocalSandbox, &spec).required()?;

    assert_eq!(
        outcome.status.code(),
        Some(crate::support::files::TRANSFER_INCOMPLETE)
    );
    assert_eq!(fs::read_to_string(&destination).required()?, "previous\n");
    assert_eq!(names_in(parent), ["origin.bundle"]);
    Ok(())
}

/// `sbx exec`だけが、届いた中身が欠けていたと答えるhost。gitはこのhostで走らせる。
struct Truncating;

impl crate::boundary::host::HostEnvironment for Truncating {
    fn command_exists(&self, _program: &str) -> bool {
        true
    }

    fn run(
        &self,
        spec: &crate::boundary::host::CommandSpec,
    ) -> crate::diagnostics::Result<crate::boundary::host::CommandOutcome> {
        if spec.program == "sbx" {
            return Ok(crate::testing::command::outcome(
                spec,
                crate::support::files::TRANSFER_INCOMPLETE,
                "",
            ));
        }
        crate::boundary::host::RealHost.run(spec)
    }
}

#[test]
fn an_incomplete_transfer_is_reported_by_its_own_reason() -> Checked {
    let sending = Sending::new()?;
    git_in(
        &sending.host,
        &["commit", "--quiet", "--allow-empty", "-m", "first"],
    )?;

    let error = sending
        .send(&Truncating)
        .refused_because("the bundle did not arrive whole")?;

    assert_eq!(error.first_id(), Some(ErrorId::BundleTransferIncomplete));
    assert_eq!(names_in(&sending.staging()), Vec::<String>::new());
    Ok(())
}
