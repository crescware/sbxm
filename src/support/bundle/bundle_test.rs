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
    root: tempfile::TempDir,
    /// bare repositoryのgit directory。
    sandbox: PathBuf,
    /// Sandbox内のmanaged worktree。
    worktree: PathBuf,
    host: PathBuf,
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

    /// Sandboxのrefをhostへ取り込む。`stamp`を退避の名前に使う。
    fn fetch(&self, stamp: &str) -> Checked<Vec<RefChange>> {
        let git_dir = self.git_dir();
        let placed = prepare_save(&LocalSandbox, NAMESPACE, &git_dir)
            .required_because("the worktree heads are placed")?;
        assert!(placed, "the sandbox has refs to save");
        let imported = import_from_sandbox(&LocalSandbox, &self.host, NAMESPACE, &git_dir, stamp);
        finish_save(&LocalSandbox, NAMESPACE, &git_dir)
            .required_because("the worktree heads are cleared")?;
        imported.required_because("the sandbox refs are imported")
    }

    /// `save_to_host`で保存する。
    fn save(&self) -> crate::diagnostics::Result<Option<Vec<RefChange>>> {
        save_to_host(&LocalSandbox, &sandbox_name()?, &self.git_dir(), &self.host)
    }

    fn host_ref(&self, reference: &str) -> Checked<String> {
        git_in(&self.host, &["rev-parse", "--verify", reference])
    }

    /// Sandboxに残った一時ref。
    fn save_refs(&self) -> Checked<String> {
        git_in(&self.worktree, &["for-each-ref", "refs/sbxm/save/"])
    }
}

/// `NAMESPACE`を名前に持つSandbox。
fn sandbox_name() -> crate::diagnostics::Result<crate::project::SandboxName> {
    let name = crate::project::SandboxName::derive(
        &crate::project::ProjectId::parse("Example-Org/Example-Repo")?.canonical(),
    );
    assert_eq!(name.as_str(), NAMESPACE);
    Ok(name)
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
    // hostに保存済みのrefが1つも無い、最初の保存。
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
    assert!(repositories.save_refs()?.is_empty());
    Ok(())
}

#[test]
fn the_host_reads_the_sandbox_over_the_ssh_that_open_uses() -> Checked {
    // hostから始まるsshだけで届く。promptで止まらず、agentもforwardingも渡さない。
    let host = crate::testing::host::FakeSbx::listing(r#"{"sandboxes":[]}"#);
    import_from_sandbox(
        &host,
        std::path::Path::new("/work/example-repo"),
        NAMESPACE,
        "/home/agent/work/example-repo/.git",
        "20260923T100000Z",
    )
    .required()?;

    let spec = host.spec(" fetch ")?;
    assert!(
        spec.args.contains(&format!(
            "ssh://{NAMESPACE}.sbx/home/agent/work/example-repo/.git"
        )),
        "{:?}",
        spec.args
    );
    assert!(
        spec.args.contains(
            &"core.sshCommand=ssh -o BatchMode=yes -o ForwardAgent=no -o ClearAllForwardings=yes"
                .to_string()
        ),
        "{:?}",
        spec.args
    );
    assert!(spec.args.contains(&"transfer.fsckObjects=true".to_string()));
    assert_eq!(
        spec.env,
        crate::boundary::host::EnvPolicy::HostRepository,
        "{spec:?}"
    );
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
fn a_repository_without_refs_has_nothing_to_save_and_keeps_what_was_saved() -> Checked {
    // 保存するものが無いSandboxから、保存済みのrefを消えたものとして退避しない。
    let repositories = Repositories::new()?;
    repositories.fetch("20260923T100000Z")?;
    let saved = repositories.host_ref(&reference("heads/main"))?;
    let empty = repositories.root.path().join("empty.git");
    git_in(
        repositories.root.path(),
        &["init", "--quiet", "--bare", "empty.git"],
    )?;

    let answer = save_to_host(
        &LocalSandbox,
        &sandbox_name()?,
        &empty.to_string_lossy(),
        &repositories.host,
    )
    .required()?;

    assert_eq!(answer, None);
    assert_eq!(repositories.host_ref(&reference("heads/main"))?, saved);
    assert!(
        git_in(
            &repositories.host,
            &["for-each-ref", &reference("archive/")]
        )?
        .is_empty()
    );
    Ok(())
}

#[test]
fn saves_in_the_same_second_archive_under_their_own_names() -> Checked {
    // 退避の名前が重なると、前の退避と同じrefを作ろうとして取り込みが失敗する。
    let repositories = Repositories::new()?;
    repositories.fetch("20260923T100000Z")?;
    let mut archived = Vec::new();
    for round in 0..2 {
        archived.push(repositories.host_ref(&reference("heads/main"))?);
        git_in(
            &repositories.worktree,
            &[
                "commit",
                "--quiet",
                "--amend",
                "--allow-empty",
                "-m",
                &format!("amended {round}"),
            ],
        )?;
        repositories.fetch("20260923T100100Z")?;
    }

    assert_eq!(
        repositories.host_ref(&reference("archive/20260923T100100Z/heads/main"))?,
        archived[0]
    );
    assert_eq!(
        repositories.host_ref(&reference("archive/20260923T100100Z-2/heads/main"))?,
        archived[1]
    );
    Ok(())
}

#[test]
fn a_sandbox_the_host_cannot_read_is_reported_by_its_own_reason_and_imports_nothing() -> Checked {
    let repositories = Repositories::new()?;
    let missing = repositories.root.path().join("missing.git");

    let error = import_from_sandbox(
        &LocalSandbox,
        &repositories.host,
        NAMESPACE,
        &missing.to_string_lossy(),
        "20260923T100000Z",
    )
    .refused_because("there is no repository to read")?;

    assert_eq!(error.first_id(), Some(ErrorId::SandboxRepositoryUnreadable));
    let diagnostic = &error.diagnostics()[0];
    assert!(!diagnostic.facts.is_empty(), "{diagnostic:?}");
    assert!(git_in(&repositories.host, &["for-each-ref", "refs/sbx"])?.is_empty());
    assert!(git_in(&repositories.host, &["for-each-ref", "refs/sbx-incoming/"])?.is_empty());
    Ok(())
}

#[test]
fn saving_to_the_host_leaves_no_temporary_refs_on_either_side() -> Checked {
    let repositories = Repositories::new()?;
    for round in 0..2 {
        repositories.commit(&format!("round {round}"))?;
        let changes = repositories
            .save()
            .required()?
            .required_because("the sandbox has refs to save")?;
        assert!(!changes.is_empty(), "round {round} saved something");
    }
    assert_eq!(
        repositories.host_ref(&reference("heads/main"))?,
        git_in(&repositories.worktree, &["rev-parse", "HEAD"])?
    );
    assert!(repositories.save_refs()?.is_empty());
    assert!(git_in(&repositories.host, &["for-each-ref", "refs/sbx-incoming/"])?.is_empty());
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

    let tips = saved_tips(&LocalSandbox, &repositories.host, &sandbox_name()?).required()?;
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
    // 取り込みはSandboxのrepositoryだけを読む。hostのrepositoryがsubmoduleを辿る設定でも、
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
fn a_save_that_cannot_be_imported_leaves_no_temporary_refs_in_the_sandbox() -> Checked {
    // 取り込みに使う一時的な名前空間を、同じ名前のrefが塞いでいる。
    let repositories = Repositories::new()?;
    git_in(
        &repositories.host,
        &["update-ref", "refs/sbx-incoming", "HEAD"],
    )?;

    repositories
        .save()
        .refused_because("the refs cannot be imported")?;

    assert!(repositories.save_refs()?.is_empty());
    assert!(git_in(&repositories.host, &["for-each-ref", "refs/sbx/"])?.is_empty());
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

#[test]
fn temporary_refs_left_by_an_interrupted_save_are_not_saved() -> Checked {
    // 一時refを置いたあと、取り込む前に止まった保存が残したもの。次の保存は、今ある
    // worktreeのHEADだけを置き直す。
    let repositories = Repositories::new()?;
    git_in(
        &repositories.worktree,
        &["update-ref", "refs/sbxm/save/gone", "HEAD"],
    )?;

    repositories.save().required()?;

    assert!(repositories.host_ref(&reference("worktrees/gone")).is_err());
    assert!(
        repositories
            .host_ref(&reference("worktrees/example-repo.tree-0"))
            .is_ok()
    );
    assert!(repositories.save_refs()?.is_empty());
    Ok(())
}

/// `PLACE_SAVE_REFS`を、一部の起動だけ振る舞いを変える`git`を`PATH`の先頭に置いて走らせる。
///
/// `case`は、`git`へ渡った引数全体に対する`sh`の`case`の枝である。どの枝にも
/// 当たらなければ、本物の`git`を走らせる。
struct Placing {
    dir: tempfile::TempDir,
}

impl Placing {
    fn new(case: &str) -> Checked<Placing> {
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
        Ok(Placing { dir })
    }

    fn output(&self, repositories: &Repositories) -> Checked<std::process::Output> {
        std::process::Command::new("sh")
            .args(["-c", PLACE_SAVE_REFS, "sh"])
            .arg(repositories.git_dir())
            .env(
                "PATH",
                format!(
                    "{}:{}",
                    self.dir.path().join("bin").display(),
                    std::env::var("PATH").unwrap_or_default()
                ),
            )
            .output()
            .required()
    }
}

#[test]
fn a_git_failure_while_gathering_refs_is_not_taken_as_nothing_to_save() -> Checked {
    // 失敗を読み落とすと、worktreeのHEADを欠いた保存や、保存するものが無いという
    // 答えになる。どちらも、保存できていないことを隠す。
    let repositories = Repositories::new()?;
    for case in [
        r#"*" worktree list "*) exit 1 ;;"#,
        r#"*" for-each-ref --count=1 "*) exit 1 ;;"#,
    ] {
        let output = Placing::new(case)?.output(&repositories)?;
        assert!(!output.status.success(), "{case}: {output:?}");
        assert!(output.stdout.is_empty(), "{case}: {output:?}");
    }
    Ok(())
}

#[test]
fn an_answer_that_is_neither_ready_nor_empty_is_not_taken_as_nothing_to_save() -> Checked {
    let host = crate::testing::host::FakeSbx::listing(r#"{"sandboxes":[]}"#).answering(
        &format!("exec {NAMESPACE} -- sh -c {PLACE_SAVE_REFS} sh /git"),
        0,
        "",
    );

    let error = prepare_save(&host, NAMESPACE, "/git").refused_because("an empty answer")?;

    assert_eq!(error.first_id(), Some(ErrorId::ExternalOutputUnparseable));
    Ok(())
}

#[test]
fn a_save_with_nothing_new_changes_nothing_and_a_new_tag_is_saved() -> Checked {
    let repositories = Repositories::new()?;

    assert!(!repositories.save().required()?.required()?.is_empty());
    assert_eq!(repositories.save().required()?, Some(Vec::new()));
    assert!(repositories.save_refs()?.is_empty());

    // objectが増えなくても、refの名前が変われば保存する。
    git_in(&repositories.worktree, &["tag", "marked"])?;
    assert_eq!(
        repositories.save().required()?,
        Some(vec![RefChange::Created {
            reference: reference("tags/marked")
        }])
    );
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

/// Sandboxのbare repositoryの場所（`from`）を、このhostの`to`へ読み替えて走らせるhost。
///
/// 起動は`LocalSandbox`へ渡す。`to`が無ければ、Sandboxの中の起動はすべて失敗する。
struct Relocated {
    from: String,
    to: Option<String>,
}

impl crate::boundary::host::HostEnvironment for Relocated {
    fn command_exists(&self, _program: &str) -> bool {
        true
    }

    fn run(
        &self,
        spec: &crate::boundary::host::CommandSpec,
    ) -> crate::diagnostics::Result<crate::boundary::host::CommandOutcome> {
        let Some(to) = &self.to else {
            if spec.program == "sbx" {
                return Ok(crate::testing::command::outcome(spec, 1, ""));
            }
            return LocalSandbox.run(spec);
        };
        let mut moved = spec.clone();
        moved.args = spec
            .args
            .iter()
            .map(|arg| arg.replace(&self.from, to))
            .collect();
        LocalSandbox.run(&moved)
    }
}

impl Relocated {
    /// `metadata`のSandboxのbare repositoryを、`repositories`のものへ読み替える。
    fn onto(
        metadata: &crate::metadata::ProjectMetadata,
        repositories: Option<&Repositories>,
    ) -> Relocated {
        Relocated {
            from: crate::project::SandboxLayout::new(metadata.canonical_id()).bare_git_dir(),
            to: repositories.map(Repositories::git_dir),
        }
    }
}

/// hostのrepositoryを、`host`の場所に登録した案件のmetadata。
fn local_metadata(host: &std::path::Path) -> Checked<crate::metadata::ProjectMetadata> {
    Ok(crate::metadata::ProjectMetadata {
        repository: crate::repository::RepositoryIdentity::local(
            host.join(".git").to_str().required()?,
            "host",
        )
        .required_because("a local repository")?,
        ..crate::testing::metadata::attached("example-org", "example-repo")?
    })
}

#[test]
fn a_local_project_saves_its_commits_on_its_own_before_the_sandbox_goes() -> Checked {
    let repositories = Repositories::new()?;
    let paths = crate::testing::repository::project_paths(repositories.root.path())?;
    let metadata = local_metadata(&repositories.host)?;

    let saved = auto_save(
        &Relocated::onto(&metadata, Some(&repositories)),
        &paths,
        &metadata,
        &mut crate::design::SilentProgress,
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

    let saved = auto_save(
        &Relocated::onto(&metadata, None),
        &paths,
        &metadata,
        &mut crate::design::SilentProgress,
    );

    let AutoSaved::Failed(warning) = saved else {
        return Err(crate::testing::outcome::Unmet::new(format!("{saved:?}")));
    };
    assert_eq!(warning.description.id, "auto-save-failed");
    assert_eq!(warning.commands.len(), 1, "{warning:?}");
    assert!(!warning.facts.is_empty(), "{warning:?}");
    // 何が起きたかは、ErrorIdだけでなく診断の一文で示す。
    assert!(!warning.guidance.is_empty(), "{warning:?}");
    Ok(())
}

#[test]
fn a_github_project_is_left_to_its_origin() -> Checked {
    let repositories = Repositories::new()?;
    let paths = crate::testing::repository::project_paths(repositories.root.path())?;
    let metadata = crate::testing::metadata::attached("example-org", "example-repo")?;

    let saved = auto_save(
        &Relocated::onto(&metadata, None),
        &paths,
        &metadata,
        &mut crate::design::SilentProgress,
    );

    assert!(matches!(saved, AutoSaved::Nothing), "{saved:?}");
    Ok(())
}

#[test]
fn a_bundle_stopped_by_a_signal_while_arriving_leaves_nothing_behind() -> Checked {
    use std::io::Write;

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
    let received = || -> u64 {
        fs::read_dir(&parent).map_or(0, |entries| {
            entries
                .flatten()
                .filter_map(|entry| entry.metadata().ok())
                .map(|metadata| metadata.len())
                .sum()
        })
    };
    // 一時fileができただけでは、まだtrapを置いていないことがある。`cat`が1 byteを
    // 書いたのを見てから止める。負荷のかかったmachineでも打ち切らないよう長く待つ。
    child
        .stdin
        .as_mut()
        .required()?
        .write_all(b"w")
        .required()?;
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    while received() == 0 && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        received(),
        1,
        "the script started receiving: {:?}",
        child.try_wait()
    );

    let pid = rustix::process::Pid::from_child(&child);
    rustix::process::kill_process(pid, rustix::process::Signal::TERM).required()?;
    child.wait().required()?;

    assert_eq!(staged(), 0);
    assert!(!destination.exists());
    Ok(())
}

/// 保存済みの名前空間を持つhostのrepositoryと、そこに作る3つのcommit。
///
/// `main`をcheckoutしている。commitは`first`、`second`、`third`の順に積む。
struct Reflecting {
    _root: tempfile::TempDir,
    host: PathBuf,
    first: String,
    second: String,
    third: String,
}

impl Reflecting {
    fn new() -> Checked<Reflecting> {
        let root = tempfile::tempdir().required()?;
        let host = root.path().join("host");
        fs::create_dir(&host).required()?;
        git_in(&host, &["init", "--quiet"])?;
        let mut commits = Vec::new();
        for message in ["first", "second", "third"] {
            git_in(
                &host,
                &["commit", "--quiet", "--allow-empty", "-m", message],
            )?;
            commits.push(git_in(&host, &["rev-parse", "HEAD"])?);
        }
        let [first, second, third] = <[String; 3]>::try_from(commits)
            .map_err(|_| crate::testing::outcome::Unmet::new("three commits".to_string()))?;
        Ok(Reflecting {
            _root: root,
            host,
            first,
            second,
            third,
        })
    }

    /// Sandboxから保存したことにするref。`kind_and_name`は`heads/main`のような形。
    fn saved(&self, kind_and_name: &str, commit: &str) -> Checked {
        git_in(
            &self.host,
            &["update-ref", &reference(kind_and_name), commit],
        )?;
        Ok(())
    }

    /// hostのref。
    fn set(&self, reference: &str, commit: &str) -> Checked {
        git_in(&self.host, &["update-ref", reference, commit])?;
        Ok(())
    }

    /// `parent`の上に、hostのどのbranchにも無いcommitを作る。
    fn aside(&self, parent: &str, message: &str) -> Checked<String> {
        let tree = git_in(&self.host, &["rev-parse", &format!("{parent}^{{tree}}")])?;
        git_in(
            &self.host,
            &["commit-tree", &tree, "-p", parent, "-m", message],
        )
    }

    fn reflect(&self) -> Checked<Vec<Reflected>> {
        reflect_saved(&LocalSandbox, &self.host, NAMESPACE).required()
    }

    fn at(&self, reference: &str) -> Checked<String> {
        git_in(&self.host, &["rev-parse", "--verify", reference])
    }
}

fn reflected(reference: &str, result: ReflectResult) -> Reflected {
    Reflected {
        reference: reference.to_string(),
        result,
    }
}

#[test]
fn saved_branches_and_tags_reach_the_host_by_the_rules_of_a_git_push() -> Checked {
    let host = Reflecting::new()?;
    // 新しいbranchとtag、早送り、遅れているだけ、分岐、同じ名前で別の先を指すtag。
    host.saved("heads/topic", &host.third)?;
    host.set("refs/heads/side", &host.first)?;
    host.saved("heads/side", &host.third)?;
    host.set("refs/heads/back", &host.third)?;
    host.saved("heads/back", &host.first)?;
    let forked = host.aside(&host.second, "forked")?;
    host.set("refs/heads/fork", &host.third)?;
    host.saved("heads/fork", &forked)?;
    host.saved("tags/fresh", &host.second)?;
    host.set("refs/tags/clash", &host.first)?;
    host.saved("tags/clash", &host.second)?;
    // 変わらないrefは返さない。
    host.set("refs/heads/same", &host.second)?;
    host.saved("heads/same", &host.second)?;

    let mut results = host.reflect()?;
    results.sort_by(|left, right| left.reference.cmp(&right.reference));

    assert_eq!(
        results,
        vec![
            reflected("refs/heads/back", ReflectResult::Behind),
            reflected("refs/heads/fork", ReflectResult::Diverged),
            reflected("refs/heads/side", ReflectResult::Updated),
            reflected("refs/heads/topic", ReflectResult::Created),
            reflected("refs/tags/clash", ReflectResult::Exists),
            reflected("refs/tags/fresh", ReflectResult::Created),
        ]
    );
    assert_eq!(host.at("refs/heads/side")?, host.third);
    assert_eq!(host.at("refs/heads/topic")?, host.third);
    assert_eq!(host.at("refs/heads/back")?, host.third);
    assert_eq!(host.at("refs/heads/fork")?, host.third);
    assert_eq!(host.at("refs/tags/clash")?, host.first);
    Ok(())
}

#[test]
fn the_checked_out_branch_follows_the_host_repository_settings() -> Checked {
    // 既定では、hostのgitはcheckoutしているbranchを動かさない。hostのrepositoryが
    // `updateInstead`を選んでいれば、変更の無い作業treeごと進める。
    let host = Reflecting::new()?;
    let ahead = host.aside(&host.third, "ahead")?;
    host.saved("heads/master", &ahead)?;
    host.saved("heads/main", &ahead)?;
    let current = git_in(&host.host, &["symbolic-ref", "--short", "HEAD"])?;

    let results = host.reflect()?;
    assert!(
        results.contains(&reflected(
            &format!("refs/heads/{current}"),
            ReflectResult::CheckedOut
        )),
        "{results:?}"
    );
    assert_eq!(host.at("HEAD")?, host.third);

    git_in(
        &host.host,
        &["config", "receive.denyCurrentBranch", "updateInstead"],
    )?;
    let results = host.reflect()?;
    assert!(
        results.contains(&reflected(
            &format!("refs/heads/{current}"),
            ReflectResult::Updated
        )),
        "{results:?}"
    );
    assert_eq!(host.at("HEAD")?, ahead);
    assert!(git_in(&host.host, &["status", "--porcelain"])?.is_empty());
    Ok(())
}

#[test]
fn the_host_repository_hooks_decide_as_for_any_push_but_pre_push_does_not_run() -> Checked {
    // 受け取る側のhookは、hostのrepositoryの規則である。送る側の`pre-push`は、別の
    // repositoryへ送る前の確認であり、自分自身への反映には関わらない。
    use std::os::unix::fs::PermissionsExt;

    let host = Reflecting::new()?;
    host.saved("heads/topic", &host.third)?;
    let hooks = host.host.join(".git/hooks");
    fs::create_dir_all(&hooks).required()?;
    for hook in ["pre-push", "pre-receive"] {
        let path = hooks.join(hook);
        fs::write(&path, "#!/bin/sh\nexit 1\n").required()?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).required()?;
    }

    let results = host.reflect()?;

    assert_eq!(
        results,
        vec![reflected(
            "refs/heads/topic",
            ReflectResult::Refused {
                reason: "pre-receive hook declined".to_string()
            }
        )]
    );
    assert!(host.at("refs/heads/topic").is_err());
    fs::remove_file(hooks.join("pre-receive")).required()?;
    assert_eq!(
        host.reflect()?,
        vec![reflected("refs/heads/topic", ReflectResult::Created)]
    );
    Ok(())
}

#[test]
fn nothing_saved_reflects_nothing() -> Checked {
    let host = Reflecting::new()?;
    assert!(host.reflect()?.is_empty());
    Ok(())
}

#[test]
fn a_push_git_could_not_run_is_an_error_rather_than_nothing_reflected() -> Checked {
    // 名前空間をrefにできないSandbox名では、gitはrefごとの答えを持たずに終わる。
    let host = Reflecting::new()?;

    let error = reflect_saved(&LocalSandbox, &host.host, "not a ref name")
        .refused_because("git cannot read the refspec")?;

    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandFailed));
    Ok(())
}

#[test]
fn a_ref_line_git_did_not_write_is_not_read_as_a_result() -> Checked {
    let push = format!(
        "push --porcelain --no-verify . refs/sbx/{NAMESPACE}/heads/*:refs/heads/* refs/sbx/{NAMESPACE}/tags/*:refs/tags/*"
    );
    for line in [
        "!\trefs/heads/main\t[rejected] (non-fast-forward)\n",
        "?\trefs/sbx/x/heads/main:refs/heads/main\t[odd]\n",
    ] {
        let host = crate::testing::host::FakeSbx::listing(r#"{"sandboxes":[]}"#).answering(
            &push,
            1,
            &format!("To .\n{line}Done\n"),
        );

        let error = reflect_saved(&host, std::path::Path::new("/work/app"), NAMESPACE)
            .refused_because("the line cannot be read")?;

        assert_eq!(
            error.first_id(),
            Some(ErrorId::ExternalOutputUnparseable),
            "{line}"
        );
    }
    Ok(())
}
