use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, UNIX_EPOCH};

use crate::diagnostics::ErrorId;

use crate::testing::outcome::{Checked, Refused, Required, Unmet};
use crate::testing::repository::git_in;
use crate::testing::sandbox::LocalSandbox;

use super::*;

const NAMESPACE: &str = "sbxm-example-org-example-repo-99a40327a69b";

/// Sandboxの中を模したbare repositoryと、hostのrepository。
struct Repositories {
    root: tempfile::TempDir,
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
            root,
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

    /// Sandboxからbundleを受け取る。
    ///
    /// 保存済みのものと比べず、毎回bundleを運ばせる。取り込みの振る舞いを確かめるため。
    fn receive(&self, label: &str) -> Checked<ReceivedBundle> {
        match receive_bundle(
            &LocalSandbox,
            "sbxm-example",
            &self.git_dir(),
            &self.bundles,
            label,
            &BTreeMap::new(),
        )
        .required()?
        {
            Receipt::Bundle(received) => Ok(received),
            other => Err(Unmet::new(format!(
                "the sandbox has refs to save: {other:?}"
            ))),
        }
    }

    /// Sandboxからbundleを受け取り、hostへ取り込む。
    fn fetch(&self, label: &str) -> Checked<Vec<RefChange>> {
        let received = self.receive(label)?;
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

    let received = receive_bundle(
        &LocalSandbox,
        "sbxm-example",
        &git_dir,
        &bundles,
        "stamp",
        &BTreeMap::new(),
    )
    .required()?;
    assert_eq!(received, Receipt::Nothing);
    assert!(
        fs::read_dir(&bundles).required()?.next().is_none(),
        "no empty bundle is kept"
    );
    Ok(())
}

#[test]
fn a_bundle_received_in_the_same_second_gets_its_own_name() -> Checked {
    let repositories = Repositories::new()?;
    let first = repositories.receive("20260923T100000Z")?;
    let second = repositories.receive("20260923T100000Z")?;
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
    // 番号でない`-`の後ろは、名前の一部として並べる。
    for name in [
        "0-copy.bundle",
        "1.bundle",
        "2.bundle",
        "3.bundle",
        "4.bundle",
        "notes.txt",
    ] {
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

    Ok(())
}

#[test]
fn a_sandbox_branch_named_like_the_archive_is_saved_again_and_again() -> Checked {
    // `archive/`で始まるbranchも、Sandboxの作業である。退避先と取り違えると、2度目の
    // 取り込みが同じrefをもう一度作ろうとして、以後の保存がすべて失敗した。
    let repositories = Repositories::new()?;
    git_in(
        &repositories.worktree,
        &["branch", "--quiet", "archive/old"],
    )?;
    repositories.fetch("20260923T100000Z")?;

    git_in(
        &repositories.worktree,
        &["checkout", "--quiet", "archive/old"],
    )?;
    let moved = repositories.commit("moved")?;
    let changes = repositories.fetch("20260923T100100Z")?;

    assert!(
        changes.contains(&RefChange::Updated {
            reference: reference("heads/archive/old")
        }),
        "{changes:?}"
    );
    assert_eq!(
        repositories.host_ref(&reference("heads/archive/old"))?,
        moved
    );
    assert!(repositories.fetch("20260923T100200Z")?.is_empty());
    Ok(())
}

#[test]
fn a_branch_renamed_into_its_own_directory_is_saved_in_both_directions() -> Checked {
    // `topic`を消して`topic/part`を作ると、消す名前と作る名前がfileとdirectoryで
    // 重なる。1回のtransactionでは、gitは`topic`がまだあるとして作成を拒んだ。
    let repositories = Repositories::new()?;
    git_in(&repositories.worktree, &["branch", "--quiet", "topic"])?;
    repositories.fetch("20260923T100000Z")?;
    let topic = repositories.host_ref(&reference("heads/topic"))?;

    git_in(
        &repositories.worktree,
        &["branch", "--quiet", "-m", "topic", "topic/part"],
    )?;
    let changes = repositories.fetch("20260923T100100Z")?;
    assert!(
        changes.contains(&RefChange::Created {
            reference: reference("heads/topic/part")
        }),
        "{changes:?}"
    );
    assert_eq!(
        repositories.host_ref(&reference("heads/topic/part"))?,
        topic
    );
    assert_eq!(
        repositories.host_ref(&reference("archive/20260923T100100Z/heads/topic"))?,
        topic
    );

    git_in(
        &repositories.worktree,
        &["branch", "--quiet", "-m", "topic/part", "topic"],
    )?;
    let changes = repositories.fetch("20260923T100200Z")?;
    assert!(
        changes.contains(&RefChange::Created {
            reference: reference("heads/topic")
        }),
        "{changes:?}"
    );
    assert_eq!(repositories.host_ref(&reference("heads/topic"))?, topic);
    assert!(repositories.fetch("20260923T100300Z")?.is_empty());
    Ok(())
}

#[test]
fn a_host_repository_with_submodules_is_saved_to_without_reaching_their_remotes() -> Checked {
    // 取り込みはbundleだけを読む。hostのrepositoryがsubmoduleを辿る設定でも、
    // submoduleのremoteへ取りに行かない。行けば、届かないremoteで保存が失敗する。
    let repositories = Repositories::new()?;
    let library = repositories.host.with_file_name("library");
    fs::create_dir(&library).required()?;
    git_in(&library, &["init", "--quiet"])?;
    git_in(
        &library,
        &["commit", "--quiet", "--allow-empty", "-m", "library"],
    )?;
    git_in(
        &repositories.host,
        &[
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "add",
            "--quiet",
            &library.to_string_lossy(),
            "library",
        ],
    )?;
    git_in(&repositories.host, &["commit", "--quiet", "-m", "library"])?;
    git_in(
        &repositories.host,
        &["config", "fetch.recurseSubmodules", "true"],
    )?;
    fs::remove_dir_all(&library).required()?;

    let changes = repositories.fetch("20260923T100000Z")?;
    assert!(
        changes.contains(&RefChange::Created {
            reference: reference("heads/main")
        }),
        "{changes:?}"
    );
    Ok(())
}

#[test]
fn bundles_that_could_not_be_imported_do_not_pile_up() -> Checked {
    // 取り込めない状態が続いても、受け取ったbundleは直近の数件だけを残す。
    let repositories = Repositories::new()?;
    let parent = tempfile::tempdir().required()?;
    let canonical = crate::project::ProjectId::parse("Example-Org/Example-Repo")
        .required()?
        .canonical();
    let paths = crate::paths::ProjectPaths::at(parent.path(), &canonical);
    let sandbox = crate::project::SandboxName::derive(&canonical);
    // 取り込みに使う一時的な名前空間を、同じ名前のrefが塞いでいる。
    git_in(
        &repositories.host,
        &["update-ref", "refs/sbx-incoming", "HEAD"],
    )?;

    for round in 0..=KEPT_BUNDLES {
        repositories.commit(&format!("round {round}"))?;
        save_to_host(
            &LocalSandbox,
            &paths,
            &sandbox,
            &repositories.git_dir(),
            &repositories.host,
        )
        .refused_because("the bundle cannot be imported")?;
    }
    let kept = fs::read_dir(paths.bundles_dir()).required()?.count();
    assert_eq!(kept, KEPT_BUNDLES, "only the newest bundles are kept");
    Ok(())
}

#[test]
fn bundles_received_in_the_same_second_are_pruned_in_the_order_they_arrived() -> Checked {
    // 同じ秒に受け取ったbundleは番号で並ぶ。名前の文字順では`-2`が番号の無い最初の
    // ものより前に、`-10`が`-2`より前に来て、新しいものを消していた。
    let root = tempfile::tempdir().required()?;
    for name in [
        "20260923T100000Z.bundle",
        "20260923T100000Z-2.bundle",
        "20260923T100000Z-10.bundle",
        "20260923T100001Z.bundle",
    ] {
        fs::write(root.path().join(name), b"").required()?;
    }
    prune_bundles(root.path(), 2).required()?;
    let mut left: Vec<String> = fs::read_dir(root.path())
        .required()?
        .filter_map(std::result::Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    left.sort();
    assert_eq!(
        left,
        vec!["20260923T100000Z-10.bundle", "20260923T100001Z.bundle"]
    );
    Ok(())
}

#[test]
fn worktrees_sharing_a_directory_name_each_keep_their_head() -> Checked {
    // 別の場所にある同じ名前のworktreeが、先に置いたHEADのrefを上書きしない。
    let repositories = Repositories::new()?;
    let elsewhere = repositories
        .sandbox
        .parent()
        .required()?
        .with_file_name("elsewhere")
        .join("example-repo.tree-0");
    git_in(
        &repositories.worktree,
        &[
            "worktree",
            "add",
            "--quiet",
            "--detach",
            &elsewhere.to_string_lossy(),
        ],
    )?;
    git_in(
        &elsewhere,
        &["commit", "--quiet", "--allow-empty", "-m", "detached"],
    )?;
    let detached = git_in(&elsewhere, &["rev-parse", "HEAD"])?;
    let main = git_in(&repositories.worktree, &["rev-parse", "HEAD"])?;

    repositories.fetch("20260923T100000Z")?;
    let saved = git_in(
        &repositories.host,
        &[
            "for-each-ref",
            "--format=%(objectname)",
            &reference("worktrees/"),
        ],
    )?;
    let mut tips: Vec<&str> = saved.lines().collect();
    tips.sort_unstable();
    let mut expected = vec![main.as_str(), detached.as_str()];
    expected.sort_unstable();
    assert_eq!(tips, expected);
    Ok(())
}

/// `CREATE_BUNDLE`を、一部の起動だけ振る舞いを変える`git`を`PATH`の先頭に置いて走らせる。
///
/// `case`は、`git`へ渡った引数全体に対する`sh`の`case`の枝である。どの枝にも
/// 当たらなければ、本物の`git`を走らせる。
struct Creating {
    dir: tempfile::TempDir,
}

impl Creating {
    fn new(case: &str) -> Checked<Creating> {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().required()?;
        fs::create_dir(dir.path().join("bin")).required()?;
        let git = dir.path().join("bin/git");
        fs::write(
            &git,
            format!(
                "#!/bin/sh\ncase \" $* \" in\n{case}\nesac\nPATH=${{PATH#*:}}\nexec git \"$@\"\n"
            ),
        )
        .required()?;
        fs::set_permissions(&git, fs::Permissions::from_mode(0o755)).required()?;
        Ok(Creating { dir })
    }

    fn command(&self, repositories: &Repositories) -> std::process::Command {
        let mut command = std::process::Command::new("sh");
        command
            .args(["-c", CREATE_BUNDLE, "sh"])
            .arg(repositories.git_dir())
            .env(
                "PATH",
                format!(
                    "{}:{}",
                    self.dir.path().join("bin").display(),
                    std::env::var("PATH").unwrap_or_default()
                ),
            )
            .env("MARK", self.dir.path().join("mark"))
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        command
    }
}

#[test]
fn a_git_failure_while_gathering_refs_is_not_taken_as_nothing_to_save() -> Checked {
    // 失敗を読み落とすと、worktreeのHEADを欠いたbundleや、保存するものが無いという
    // 答えになる。どちらも、保存できていないことを隠す。
    let repositories = Repositories::new()?;
    for case in [
        r#"*" worktree list "*) exit 1 ;;"#,
        r#"*" for-each-ref --format=%(objectname) %(refname) "*) exit 1 ;;"#,
    ] {
        let status = Creating::new(case)?
            .command(&repositories)
            .status()
            .required()?;
        assert!(!status.success(), "{case}: {status:?}");
    }
    Ok(())
}

#[test]
fn a_bundle_stopped_by_a_signal_leaves_no_temporary_refs() -> Checked {
    // worktreeのHEADを置いた一時refは、止められても残さない。
    let repositories = Repositories::new()?;
    let creating = Creating::new(r#"*" bundle create "*) : > "$MARK"; cat > /dev/null ;;"#)?;
    let mut child = creating
        .command(&repositories)
        .stdin(std::process::Stdio::piped())
        .spawn()
        .required()?;
    let mark = creating.dir.path().join("mark");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !mark.exists() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(mark.exists(), "the script started bundling");
    let saved = || git_in(&repositories.worktree, &["for-each-ref", "refs/sbxm/save/"]);
    assert!(!saved()?.is_empty(), "the worktree heads were placed");

    let pid = rustix::process::Pid::from_child(&child);
    rustix::process::kill_process(pid, rustix::process::Signal::TERM).required()?;
    child.wait().required()?;

    assert!(saved()?.is_empty());
    Ok(())
}

#[test]
fn a_save_with_nothing_new_carries_no_bundle() -> Checked {
    // bundleは履歴全体を運ぶ。何も変えていないSandboxの保存に、それを繰り返さない。
    let repositories = Repositories::new()?;
    let parent = tempfile::tempdir().required()?;
    let canonical = crate::project::ProjectId::parse("Example-Org/Example-Repo")
        .required()?
        .canonical();
    let paths = crate::paths::ProjectPaths::at(parent.path(), &canonical);
    let sandbox = crate::project::SandboxName::derive(&canonical);
    let save = || {
        save_to_host(
            &LocalSandbox,
            &paths,
            &sandbox,
            &repositories.git_dir(),
            &repositories.host,
        )
        .required()?
        .required_because("the sandbox has refs to save")
    };
    let bundles =
        || -> Checked<usize> { Ok(fs::read_dir(paths.bundles_dir()).required()?.count()) };

    assert!(!save()?.is_empty());
    assert_eq!(bundles()?, 1);
    assert_eq!(save()?, Vec::new());
    assert_eq!(bundles()?, 1, "no bundle is carried for nothing new");
    assert!(git_in(&repositories.worktree, &["for-each-ref", "refs/sbxm/save/"])?.is_empty());

    // objectが増えなくても、refの名前が変われば運ぶ。
    git_in(&repositories.worktree, &["tag", "marked"])?;
    assert_eq!(
        save()?,
        vec![RefChange::Created {
            reference: format!("refs/sbx/{}/tags/marked", sandbox.as_str())
        }]
    );
    assert_eq!(bundles()?, 2);
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
        Some(crate::support::sandbox::TRANSFER_INCOMPLETE)
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
                crate::support::sandbox::TRANSFER_INCOMPLETE,
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

/// hostのrepositoryと、originを読み終えたばかりの作り直したSandboxを模したrepository。
struct Rebuilt {
    root: tempfile::TempDir,
    host: PathBuf,
    sandbox: PathBuf,
}

impl Rebuilt {
    fn new() -> Checked<Rebuilt> {
        let root = tempfile::tempdir().required()?;
        let host = root.path().join("host");
        fs::create_dir(&host).required()?;
        git_in(&host, &["init", "--quiet"])?;
        git_in(
            &host,
            &["commit", "--quiet", "--allow-empty", "-m", "published"],
        )?;
        let sandbox = root.path().join("sandbox.git");
        let git_dir = sandbox.to_string_lossy().into_owned();
        git_in(root.path(), &["init", "--quiet", "--bare", &git_dir])?;
        git_in(
            root.path(),
            &[
                "--git-dir",
                &git_dir,
                "remote",
                "add",
                "origin",
                &host.to_string_lossy(),
            ],
        )?;
        git_in(
            root.path(),
            &["--git-dir", &git_dir, "fetch", "--quiet", "origin"],
        )?;
        Ok(Rebuilt {
            root,
            host,
            sandbox,
        })
    }

    /// hostで`name`のbranchへcommitを1つ積み、Sandboxから保存した先端として残す。
    fn save(&self, name: &str) -> Checked<String> {
        git_in(
            &self.host,
            &["checkout", "--quiet", "-B", &format!("work-{name}")],
        )?;
        git_in(
            &self.host,
            &["commit", "--quiet", "--allow-empty", "-m", name],
        )?;
        let tip = git_in(&self.host, &["rev-parse", "HEAD"])?;
        git_in(
            &self.host,
            &[
                "update-ref",
                &format!("refs/sbx/{NAMESPACE}/heads/{name}"),
                &tip,
            ],
        )?;
        git_in(&self.host, &["checkout", "--quiet", "main"])?;
        git_in(
            &self.host,
            &["branch", "--quiet", "-D", &format!("work-{name}")],
        )?;
        Ok(tip)
    }

    fn restore(&self) -> crate::diagnostics::Result<Vec<String>> {
        restore_saved_branches(
            &LocalSandbox,
            &self.host,
            &self.root.path().join("bundles"),
            NAMESPACE,
            &self.sandbox.to_string_lossy(),
        )
    }

    fn sandbox_git(&self, args: &[&str]) -> Checked<String> {
        let git_dir = self.sandbox.to_string_lossy().into_owned();
        let mut full = vec!["--git-dir", git_dir.as_str()];
        full.extend_from_slice(args);
        git_in(self.root.path(), &full)
    }
}

#[test]
fn branches_saved_on_the_host_come_back_as_sandbox_branches() -> Checked {
    let rebuilt = Rebuilt::new()?;
    let main = rebuilt.save("main")?;
    let topic = rebuilt.save("topic")?;

    let restored = rebuilt.restore().required()?;

    assert_eq!(restored, ["main", "topic"]);
    assert_eq!(
        rebuilt.sandbox_git(&["rev-parse", "refs/heads/main"])?,
        main
    );
    assert_eq!(
        rebuilt.sandbox_git(&["rev-parse", "refs/heads/topic"])?,
        topic
    );
    // originに同じ名前があるbranchだけが、それをupstreamにする。
    assert_eq!(
        rebuilt.sandbox_git(&["rev-parse", "--symbolic-full-name", "main@{upstream}"])?,
        "refs/remotes/origin/main"
    );
    assert!(
        rebuilt
            .sandbox_git(&["config", "--get-regexp", "^branch\\.topic\\."])
            .is_err(),
        "topic has no upstream"
    );
    // 送ったbundleは残さない。
    assert!(!rebuilt.sandbox.join("sbxm/restore.bundle").exists());
    Ok(())
}

#[test]
fn a_sandbox_without_saved_branches_receives_nothing() -> Checked {
    let rebuilt = Rebuilt::new()?;

    let restored = rebuilt.restore().required()?;

    assert!(restored.is_empty());
    assert!(!rebuilt.sandbox.join("sbxm").exists());
    assert_eq!(
        rebuilt.sandbox_git(&["for-each-ref", "--format=%(refname)", "refs/heads/"])?,
        ""
    );
    Ok(())
}

/// Sandboxのbundleの手順だけに、用意したbundleのbyte列で答えるhost。gitはこのhostで走らせる。
struct ServingBundle {
    bundle: Option<Vec<u8>>,
}

impl crate::boundary::host::HostEnvironment for ServingBundle {
    fn command_exists(&self, _program: &str) -> bool {
        true
    }

    fn run(
        &self,
        spec: &crate::boundary::host::CommandSpec,
    ) -> crate::diagnostics::Result<crate::boundary::host::CommandOutcome> {
        use std::os::unix::process::ExitStatusExt;

        if spec.program != "sbx" {
            return crate::boundary::host::RealHost.run(spec);
        }
        let (status, stdout) = match &self.bundle {
            Some(bytes) => (0, bytes.clone()),
            None => (1 << 8, Vec::new()),
        };
        Ok(crate::boundary::host::CommandOutcome {
            program: spec.program.clone(),
            args: spec.args.clone(),
            working_dir: spec.working_dir.clone(),
            status: std::process::ExitStatus::from_raw(status),
            stdout,
            stderr: Vec::new(),
            stderr_lossy: false,
        })
    }
}

/// hostのrepositoryを、`host`の場所に登録した案件のmetadata。
fn local_metadata(host: &std::path::Path) -> Checked<crate::metadata::ProjectMetadata> {
    Ok(crate::metadata::ProjectMetadata {
        repository: crate::repository::RepositoryIdentity::local(host.to_str().required()?, "host")
            .required_because("a local repository")?,
        ..crate::testing::metadata::attached("example-org", "example-repo")?
    })
}

#[test]
fn a_local_project_saves_its_commits_on_its_own_before_the_sandbox_goes() -> Checked {
    let repositories = Repositories::new()?;
    let bundle = repositories.root.path().join("sandbox.bundle");
    git_in(
        repositories.root.path(),
        &[
            "--git-dir",
            &repositories.git_dir(),
            "bundle",
            "create",
            "--quiet",
            &bundle.to_string_lossy(),
            "--branches",
        ],
    )?;
    let bytes = fs::read(&bundle).required()?;
    let paths = crate::testing::repository::project_paths(repositories.root.path())?;
    let metadata = local_metadata(&repositories.host)?;

    let saved = auto_save(
        &ServingBundle {
            bundle: Some(bytes),
        },
        &paths,
        &metadata,
    );

    let AutoSaved::Saved(message) = saved else {
        return Err(crate::testing::outcome::Unmet::new(format!("{saved:?}")));
    };
    assert_eq!(message.id, "auto-save-done");
    let namespace = metadata.sandbox_name();
    git_in(
        &repositories.host,
        &[
            "rev-parse",
            "--verify",
            &format!("refs/sbx/{}/heads/main", namespace.as_str()),
        ],
    )?;
    Ok(())
}

#[test]
fn a_save_that_fails_is_a_warning_with_the_command_to_retry() -> Checked {
    let repositories = Repositories::new()?;
    let paths = crate::testing::repository::project_paths(repositories.root.path())?;
    let metadata = local_metadata(&repositories.host)?;

    let saved = auto_save(&ServingBundle { bundle: None }, &paths, &metadata);

    let AutoSaved::Failed(warning) = saved else {
        return Err(crate::testing::outcome::Unmet::new(format!("{saved:?}")));
    };
    assert_eq!(warning.description.id, "auto-save-failed");
    assert_eq!(warning.commands.len(), 1, "{warning:?}");
    assert!(!warning.facts.is_empty(), "{warning:?}");
    Ok(())
}

#[test]
fn a_github_project_is_left_to_its_origin() -> Checked {
    let repositories = Repositories::new()?;
    let paths = crate::testing::repository::project_paths(repositories.root.path())?;
    let metadata = crate::testing::metadata::attached("example-org", "example-repo")?;

    let saved = auto_save(&ServingBundle { bundle: None }, &paths, &metadata);

    assert!(matches!(saved, AutoSaved::Nothing), "{saved:?}");
    Ok(())
}

#[test]
fn a_bundle_stopped_by_a_signal_while_arriving_leaves_nothing_behind() -> Checked {
    // 受け取りの途中でsignalを受けても、書きかけの一時fileを残さない。dashはsignalで
    // 終わるshellのEXIT trapを走らせない。
    let sending = Sending::new()?;
    let destination = sending.destination();
    let parent = destination.parent().required()?.to_path_buf();
    let mut child = std::process::Command::new("sh")
        .args(["-c", PLACE_BUNDLE, "sh"])
        .arg(&destination)
        .arg(crate::hash::sha256_hex(b"whole"))
        .stdin(std::process::Stdio::piped())
        .spawn()
        .required()?;
    let staged = || -> usize { fs::read_dir(&parent).map_or(0, Iterator::count) };
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while staged() == 0 && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(staged(), 1, "the script started receiving");

    let pid = rustix::process::Pid::from_child(&child);
    rustix::process::kill_process(pid, rustix::process::Signal::TERM).required()?;
    child.wait().required()?;

    assert_eq!(staged(), 0);
    assert!(!destination.exists());
    Ok(())
}
