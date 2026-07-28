//! `sbxm add`。
//!
//! 新しいGitHub repositoryを管理対象へ登録し、構築が中断した案件には同じcommandで
//! 続きから進む。全工程を`inspect -> decide -> mutate -> verify -> record`で実行し、
//! 成功済みの成果物をrollback目的で削除しない。

use std::fs;
use std::path::PathBuf;

use crate::command::HostEnvironment;
use crate::config::GlobalConfig;
use crate::error::{Diagnostic, Error, ErrorId, Msg, Result, fail};
use crate::git;
use crate::hash::sha256_hex;
use crate::metadata::{
    self, CreationMode, MAX_WORKTREES, MIN_WORKTREES, ProjectMetadata, Provisioning,
};
use crate::msg;
use crate::paths::{
    self, ExclusiveLock, LOCK_TIMEOUT, PRIVATE_DIR_MODE, PRIVATE_FILE_MODE, PathScope, ProjectPaths,
};
use crate::project::{ProjectId, SandboxName};

use super::host_clone;

/// 案件のDockerfileを新規作成するときの初期template。
///
/// 作成後は利用者が管理するfileであり、sbxmは内容を変更しない。変更の適用は
/// `rebuild`が担当する。
const BUNDLED_DOCKERFILE: &str = include_str!("../../assets/Dockerfile");

/// `add`の入力。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddRequest {
    pub project: ProjectId,
    pub worktrees: Option<u32>,
    pub detach: Option<String>,
}

/// optionから決まる目標構成。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetConfiguration {
    pub mode: CreationMode,
    pub start_ref: Option<String>,
    pub requested_worktrees: u32,
}

impl TargetConfiguration {
    /// | 指定 | mode | start_ref | managed数 |
    /// |---|---|---|---:|
    /// | 指定なし | attached | remote default branch | 1 |
    /// | `--detach BRANCH` | detached | BRANCH | 1 |
    /// | `--worktrees N --detach BRANCH` | detached | BRANCH | N |
    pub fn from_request(request: &AddRequest) -> Result<TargetConfiguration> {
        let requested_worktrees = request.worktrees.unwrap_or(MIN_WORKTREES);
        if !(MIN_WORKTREES..=MAX_WORKTREES).contains(&requested_worktrees) {
            return fail(
                ErrorId::WorktreesOutOfRange,
                msg!(
                    "error-worktrees-out-of-range",
                    value = requested_worktrees,
                    minimum = MIN_WORKTREES,
                    maximum = MAX_WORKTREES
                ),
            );
        }

        match &request.detach {
            Some(branch) => {
                git::validate_branch_name(branch)?;
                Ok(TargetConfiguration {
                    mode: CreationMode::Detached,
                    start_ref: Some(branch.clone()),
                    requested_worktrees,
                })
            }
            None => {
                // 2個以上のmanaged worktreeは、起点branchの明示を必須とする。
                if requested_worktrees > 1 {
                    return fail(
                        ErrorId::WorktreesRequireDetach,
                        msg!("error-worktrees-require-detach"),
                    );
                }
                Ok(TargetConfiguration {
                    // attached modeのstart refはremote default branchを解決してから確定する。
                    mode: CreationMode::Attached,
                    start_ref: None,
                    requested_worktrees,
                })
            }
        }
    }
}

/// 登録を終えた案件。
///
/// project lockを保持しているため、この値が生きているあいだ同じ案件へのmutationは
/// 直列化される。
#[derive(Debug)]
pub struct Registration {
    pub paths: ProjectPaths,
    pub sandbox: SandboxName,
    pub metadata: ProjectMetadata,
    _lock: ExclusiveLock,
}

/// 案件を登録し、以後の外部mutationへ進める状態にする。
///
/// 1. 入力を検証し、既存案件との衝突検査を完了する
/// 2. owner directory、project root、`.sbxm`、`.cache`を作る
/// 3. project lockを取得する
/// 4. Dockerfileがなければbundled templateから作り、あれば内容を変えず採用する
/// 5. 目標構成を含むmetadataをatomic writeする
pub fn register(config: &GlobalConfig, request: &AddRequest) -> Result<Registration> {
    let target = TargetConfiguration::from_request(request)?;
    let canonical = request.project.canonical();
    let sandbox = SandboxName::derive(&canonical);

    // 破損した案件が1件でもあれば、一覧を部分的に信用せずここで停止する。
    let known = metadata::discover(&config.base_path)?;
    if let Some(other) = known.iter().find(|project| {
        project.metadata.canonical_id != canonical && project.metadata.sandbox_name() == sandbox
    }) {
        return fail(
            ErrorId::SandboxNameCollision,
            msg!(
                "error-sandbox-name-collision",
                sandbox = sandbox,
                projects = format!("{}, {}", canonical, other.metadata.canonical_id)
            ),
        );
    }
    if let Some(project) = known
        .iter()
        .find(|project| project.metadata.canonical_id == canonical)
    {
        // mutationの前に、保存済み目標構成との一致を判定する。
        check_continuable(&project.metadata, request)?;
    }

    let paths = ProjectPaths::derive(&config.base_path, &canonical);
    paths::ensure_directory(paths.owner_dir())?;
    paths::ensure_directory(paths.root())?;
    paths::ensure_private_dir(&paths.sbxm_dir(), PRIVATE_DIR_MODE, PathScope::ProjectPath)?;
    paths::ensure_private_dir(&paths.cache_dir(), PRIVATE_DIR_MODE, PathScope::ProjectPath)?;

    let lock = paths::acquire_exclusive_lock(
        &paths.lock_file(),
        LOCK_TIMEOUT,
        PRIVATE_FILE_MODE,
        PathScope::ProjectPath,
    )?;

    // lock取得後にmetadataを取り直し、preconditionを判定し直す。
    let stored = metadata::load(&paths)?;
    if let Some(stored) = &stored {
        check_continuable(stored, request)?;
    }

    let dockerfile_sha256 = adopt_dockerfile(&paths)?;

    let metadata = match stored {
        Some(stored) => raise_worktrees(&paths, stored, &target)?,
        None => {
            let metadata = ProjectMetadata {
                owner: request.project.owner().to_string(),
                repository: request.project.repository().to_string(),
                canonical_id: canonical,
                provisioning: Provisioning {
                    mode: target.mode,
                    start_ref: target.start_ref,
                    requested_worktrees: target.requested_worktrees,
                    dockerfile_sha256: dockerfile_sha256.clone(),
                },
                managed_worktrees: Vec::new(),
                rebuild: None,
            };
            metadata::create(&paths, &metadata)?;
            metadata
        }
    };

    Ok(Registration {
        paths,
        sandbox,
        metadata,
        _lock: lock,
    })
}

/// 登録済み案件の目標worktree数を引き上げる。
///
/// 1本で始めた案件が並行作業を必要とすることは普通に起きる。作り直さずに増やせる手段が
/// ないと、`destroy`してから`add`し直すことになり、Sandbox内のrepositoryごと失われる。
///
/// 引き上げるのはmetadataの目標値だけである。worktreeそのものは`prepare`が作る。
/// 減らす指定はここへ来ない。`check_continuable`が先に拒否している。
fn raise_worktrees(
    paths: &ProjectPaths,
    mut stored: ProjectMetadata,
    target: &TargetConfiguration,
) -> Result<ProjectMetadata> {
    if target.requested_worktrees <= stored.provisioning.requested_worktrees {
        return Ok(stored);
    }
    stored.provisioning.requested_worktrees = target.requested_worktrees;
    metadata::update(paths, &stored)?;
    Ok(stored)
}

/// `add`の結果。
///
/// 案件を管理下へ置き、host cloneを用意したところまでを示す。Sandboxはまだ存在せず、
/// 構築は`prepare`が行う。
#[derive(Debug, Clone)]
pub struct AddOutput {
    pub project: String,
    /// 構築で使うSandbox名。canonical project IDから決まり、この時点では未作成。
    pub sandbox: String,
    pub mode: CreationMode,
    /// 起点branch。attached modeは構築時にremoteから解決するため`None`のことがある。
    pub start_ref: Option<String>,
    pub requested_worktrees: u32,
    pub host_clone: PathBuf,
    /// 既に登録済みだったか。
    pub already_registered: bool,
    /// この実行が目標worktree数を引き上げた場合の、引き上げ前の値。
    ///
    /// 引き上げが起きたことは`already_registered`からは読めない。増えた分のworktreeは
    /// まだ存在せず、`prepare`が作る。
    pub raised_worktrees_from: Option<u32>,
    pub warnings: Vec<Msg>,
}

/// 案件を管理下へ置き、host cloneを用意する。
///
/// Sandboxは作らない。Sandbox名はcanonical project IDから決まるため、この時点で
/// 確定して呼び出し側へ返せる。GitHub tokenの登録先がその名前になる。
pub fn run(
    config: &GlobalConfig,
    request: &AddRequest,
    host: &dyn HostEnvironment,
) -> Result<AddOutput> {
    let paths = ProjectPaths::derive(&config.base_path, &request.project.canonical());
    let before = metadata::load(&paths)?;
    let already_registered = before.is_some();
    let worktrees_before = before.map(|stored| stored.provisioning.requested_worktrees);

    let registration = register(config, request)?;
    // host cloneは利用者のSSH鍵でhost上から取る。Sandboxのsecretは要らない。
    let clone = host_clone::ensure(host, &registration.paths, &request.project)?;

    let provisioning = &registration.metadata.provisioning;
    Ok(AddOutput {
        project: registration.metadata.display_id(),
        sandbox: registration.sandbox.as_str().to_string(),
        mode: provisioning.mode,
        start_ref: provisioning.start_ref.clone(),
        requested_worktrees: provisioning.requested_worktrees,
        host_clone: clone.path,
        already_registered,
        raised_worktrees_from: worktrees_before
            .filter(|before| *before < provisioning.requested_worktrees),
        warnings: Vec::new(),
    })
}

/// 現在のDockerfileのSHA-256。
///
/// `add`が採用したfileを、以後の工程が世代の正本として読む。
pub fn current_dockerfile_hash(paths: &ProjectPaths) -> Result<String> {
    let path = paths.dockerfile();
    if !paths::regular_file_exists(&path, PathScope::ProjectPath)? {
        return Err(Error::new(
            ErrorId::ProjectPathUnreadable,
            msg!(
                "error-project-path-unreadable",
                path = paths::display(&path),
                detail = "the Dockerfile of this project is absent"
            ),
        ));
    }
    let contents = std::fs::read(&path)
        .map_err(|error| PathScope::ProjectPath.unreadable_error(&path, &error.to_string()))?;
    Ok(sha256_hex(&contents))
}

/// 保存済みmetadataを持つ案件で、この`add`が構築を続けてよいかを判定する。
///
/// 省略されたoptionは保存値を使う。指定されたoptionは保存値との完全一致を要求する。
fn check_continuable(stored: &ProjectMetadata, request: &AddRequest) -> Result<()> {
    let display_id = stored.display_id();

    if stored.rebuild.is_some() {
        // 世代の切替中であり、初回構築の継続とは別の工程が必要になる。
        return Err(Error::single(
            Diagnostic::new(
                ErrorId::RebuildIntentPending,
                msg!("error-rebuild-intent-pending", project = display_id),
            )
            .remediation(msg!(
                "remediation-run-rebuild",
                command = format!("sbxm rebuild {display_id}")
            )),
        ));
    }

    let provisioning = &stored.provisioning;
    let mismatch = |requested: String, stored: String| {
        Err(Error::single(
            Diagnostic::new(
                ErrorId::TargetConfigurationMismatch,
                msg!(
                    "error-target-configuration-mismatch",
                    project = display_id,
                    requested = requested,
                    stored = stored
                ),
            )
            .remediation(msg!(
                "remediation-target-configuration-mismatch",
                command = format!("sbxm add {display_id}")
            )),
        ))
    };

    if let Some(branch) = &request.detach {
        let stored_branch = provisioning.start_ref.clone().unwrap_or_default();
        if provisioning.mode != CreationMode::Detached || stored_branch != *branch {
            return mismatch(
                format!("{} {branch}", CreationMode::Detached),
                format!("{} {stored_branch}", provisioning.mode),
            );
        }
    }
    // 増設は目標構成の引き上げとして受け付ける。減らす指定だけを不一致として扱う。
    // worktreeを減らすことはcheckoutされた作業を消すことであり、`destroy`と同じ重さの
    // 確認が要る。継続の副作用として起こしてよい変更ではない。
    if let Some(worktrees) = request.worktrees
        && worktrees < provisioning.requested_worktrees
    {
        return mismatch(
            format!("{worktrees} worktrees"),
            format!("{} worktrees", provisioning.requested_worktrees),
        );
    }
    Ok(())
}

/// Dockerfileを採用し、そのSHA-256を返す。
///
/// 既存fileは利用者が管理・編集するものとして内容を変更せず採用する。
fn adopt_dockerfile(paths: &ProjectPaths) -> Result<String> {
    let path = paths.dockerfile();
    if paths::regular_file_exists(&path, PathScope::ProjectPath)? {
        let contents = fs::read(&path)
            .map_err(|error| PathScope::ProjectPath.unreadable_error(&path, &error.to_string()))?;
        return Ok(sha256_hex(&contents));
    }
    paths::atomic_create(&path, BUNDLED_DOCKERFILE, PRIVATE_FILE_MODE)?;
    Ok(sha256_hex(BUNDLED_DOCKERFILE.as_bytes()))
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::config::{GitIdentity, GlobalConfig};
    use crate::i18n::Locale;
    use crate::metadata::RebuildIntent;
    use crate::paths::AbsoluteBasePath;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
    use std::time::Duration;

    fn setup() -> (tempfile::TempDir, GlobalConfig) {
        let dir = tempfile::tempdir().expect("temporary base path");
        let config = GlobalConfig {
            language: Locale::En,
            base_path: AbsoluteBasePath::new(dir.path()).expect("valid base path"),
            git: GitIdentity {
                user_name: "Example User".into(),
                user_email: "user@example.com".into(),
            },
            files: Vec::new(),
        };
        (dir, config)
    }

    pub fn request(project: &str, worktrees: Option<u32>, detach: Option<&str>) -> AddRequest {
        AddRequest {
            project: ProjectId::parse(project).expect("valid project id"),
            worktrees,
            detach: detach.map(|value| value.to_string()),
        }
    }

    fn mode_of(path: &Path) -> u32 {
        fs::metadata(path)
            .expect("the path exists")
            .permissions()
            .mode()
            & 0o777
    }

    pub const COMMIT: &str = "9f5b1c5a2b6d4e8f0a1b2c3d4e5f60718293a4b5";

    /// `sbx ls`だけを答え、Sandbox内のcommandは成功として扱うhost。
    #[test]
    fn registering_a_project_creates_the_documented_layout() {
        let (_dir, config) = setup();
        let registration =
            register(&config, &request("Example-Org/Example-Repo", None, None)).expect("register");

        let paths = &registration.paths;
        assert!(paths.root().is_dir());
        assert_eq!(mode_of(&paths.sbxm_dir()), PRIVATE_DIR_MODE);
        assert_eq!(mode_of(&paths.cache_dir()), PRIVATE_DIR_MODE);
        assert_eq!(mode_of(&paths.metadata_file()), PRIVATE_FILE_MODE);
        assert_eq!(mode_of(&paths.dockerfile()), PRIVATE_FILE_MODE);

        // 表示にはGitHub上の表記、突き合わせにはcanonical形式を使う。
        let metadata = &registration.metadata;
        assert_eq!(metadata.display_id(), "Example-Org/Example-Repo");
        assert_eq!(
            metadata.canonical_id.to_string(),
            "example-org/example-repo"
        );
        assert_eq!(
            registration.sandbox.as_str(),
            metadata.sandbox_name().as_str()
        );

        let stored = metadata::load(paths).expect("load").expect("present");
        assert_eq!(&stored, metadata);
        assert_eq!(stored.provisioning.mode, CreationMode::Attached);
        assert_eq!(stored.provisioning.start_ref, None);
        assert_eq!(stored.provisioning.requested_worktrees, 1);
        assert_eq!(
            stored.provisioning.dockerfile_sha256,
            current_dockerfile_hash(&registration.paths).expect("the adopted Dockerfile")
        );
        assert!(stored.managed_worktrees.is_empty());
    }

    #[test]
    fn the_bundled_dockerfile_is_written_once_and_never_edited_again() {
        let (_dir, config) = setup();
        let registration =
            register(&config, &request("example-org/example-repo", None, None)).expect("register");
        let dockerfile = registration.paths.dockerfile();
        assert_eq!(
            fs::read_to_string(&dockerfile).unwrap(),
            BUNDLED_DOCKERFILE,
            "a new project starts from the bundled template"
        );
        drop(registration);

        fs::write(&dockerfile, "FROM scratch\n").unwrap();
        let registration =
            register(&config, &request("example-org/example-repo", None, None)).expect("register");
        assert_eq!(
            fs::read_to_string(&dockerfile).unwrap(),
            "FROM scratch\n",
            "an edited Dockerfile belongs to the user"
        );
        assert_eq!(
            current_dockerfile_hash(&registration.paths).expect("the adopted Dockerfile"),
            sha256_hex(b"FROM scratch\n"),
            "the current content decides the current hash"
        );
        assert_eq!(
            registration.metadata.provisioning.dockerfile_sha256,
            sha256_hex(BUNDLED_DOCKERFILE.as_bytes()),
            "the applied generation stays as recorded until an image is built"
        );
    }

    #[test]
    fn the_bundled_dockerfile_meets_the_rules_it_ships_under() {
        assert!(
            BUNDLED_DOCKERFILE.contains(
                "docker.io/docker/sandbox-templates:shell-docker@sha256:39cf20eca861ec92747487af6197f6d916f774bdb98245d267dbd8dfd3debb05"
            ),
            "the base image stays pinned by digest"
        );
        for tool in [
            "git",
            "openssh-client",
            "coreutils",
            "ca-certificates",
            "curl",
            "wget",
            "gh",
            "jq",
        ] {
            assert!(
                BUNDLED_DOCKERFILE.contains(tool),
                "the fixed tool set installs {tool}"
            );
        }
        assert!(BUNDLED_DOCKERFILE.contains("WORKDIR /home/agent/work"));
        assert!(BUNDLED_DOCKERFILE.contains("-o agent -g agent"));
        assert!(
            !BUNDLED_DOCKERFILE.contains("GH_TOKEN"),
            "no token, real or sentinel, is written into the image"
        );
        for line in BUNDLED_DOCKERFILE.lines() {
            let instruction = line.trim_start();
            assert!(
                !instruction.starts_with("COPY ") && !instruction.starts_with("ADD "),
                "the build context is empty, so nothing can be copied into the image: {line}"
            );
        }
    }

    #[test]
    fn the_options_decide_the_target_configuration() {
        let (_dir, config) = setup();
        let cases = [
            ("one/repo", None, None, CreationMode::Attached, None, 1),
            ("two/repo", Some(1), None, CreationMode::Attached, None, 1),
            (
                "three/repo",
                None,
                Some("develop"),
                CreationMode::Detached,
                Some("develop"),
                1,
            ),
            (
                "four/repo",
                Some(1),
                Some("develop"),
                CreationMode::Detached,
                Some("develop"),
                1,
            ),
            (
                "five/repo",
                Some(3),
                Some("develop"),
                CreationMode::Detached,
                Some("develop"),
                3,
            ),
        ];

        for (project, worktrees, detach, mode, start_ref, count) in cases {
            let registration =
                register(&config, &request(project, worktrees, detach)).expect("register");
            let provisioning = &registration.metadata.provisioning;
            assert_eq!(provisioning.mode, mode, "{project}");
            assert_eq!(provisioning.start_ref.as_deref(), start_ref, "{project}");
            assert_eq!(provisioning.requested_worktrees, count, "{project}");
        }

        // 2個以上のmanaged worktreeは起点branchの明示を必須とする。
        let error = register(&config, &request("six/repo", Some(2), None))
            .expect_err("two worktrees need an explicit branch");
        assert_eq!(error.first_id(), Some(ErrorId::WorktreesRequireDetach));
    }

    #[test]
    fn an_unusable_start_branch_stops_before_anything_is_created() {
        let (dir, config) = setup();
        let error = register(
            &config,
            &request("example-org/example-repo", None, Some("-x")),
        )
        .expect_err("a branch that could be read as an option is refused");
        assert_eq!(error.first_id(), Some(ErrorId::InvalidBranchName));
        assert_eq!(
            fs::read_dir(dir.path()).unwrap().count(),
            0,
            "nothing may be created before the input is accepted"
        );
    }

    #[test]
    fn re_running_add_without_options_continues_from_the_stored_target() {
        let (_dir, config) = setup();
        let first = register(
            &config,
            &request("example-org/example-repo", Some(3), Some("develop")),
        )
        .expect("register");
        let before = fs::read_to_string(first.paths.metadata_file()).unwrap();
        drop(first);

        let again =
            register(&config, &request("example-org/example-repo", None, None)).expect("re-run");
        assert_eq!(again.metadata.provisioning.requested_worktrees, 3);
        assert_eq!(
            again.metadata.provisioning.start_ref.as_deref(),
            Some("develop")
        );
        assert_eq!(
            fs::read_to_string(again.paths.metadata_file()).unwrap(),
            before,
            "a re-run must not rewrite the stored target"
        );
    }

    #[test]
    fn a_registered_project_can_ask_for_more_worktrees_than_it_started_with() {
        let (_dir, config) = setup();
        let first = register(
            &config,
            &request("example-org/example-repo", Some(1), Some("develop")),
        )
        .expect("register");
        drop(first);

        // 1本で始めた案件が並行作業を必要とすることは普通に起きる。作り直さずに増やせる。
        let grown = register(
            &config,
            &request("example-org/example-repo", Some(3), Some("develop")),
        )
        .expect("asking for more is a continuation, not a disagreement");
        assert_eq!(grown.metadata.provisioning.requested_worktrees, 3);
        let stored_after_growth = fs::read_to_string(grown.paths.metadata_file()).unwrap();
        drop(grown);

        // 増えた分はまだ作られていない。作るのは`prepare`である。
        let again = register(&config, &request("example-org/example-repo", None, None))
            .expect("a re-run without options continues from the raised target");
        assert_eq!(
            again.metadata.provisioning.requested_worktrees, 3,
            "omitting the option must not read as a request for one worktree"
        );
        assert_eq!(
            fs::read_to_string(again.paths.metadata_file()).unwrap(),
            stored_after_growth,
            "a re-run must not rewrite the stored target"
        );
    }

    #[test]
    fn options_that_disagree_with_the_stored_target_stop_the_run() {
        let (_dir, config) = setup();
        let first = register(
            &config,
            &request("example-org/example-repo", Some(3), Some("develop")),
        )
        .expect("register");
        let before = fs::read_to_string(first.paths.metadata_file()).unwrap();
        drop(first);

        // 完全に一致するoptionは受け付ける。
        register(
            &config,
            &request("example-org/example-repo", Some(3), Some("develop")),
        )
        .expect("the same options continue the build");

        for (worktrees, detach) in [
            (Some(2), Some("develop")),
            (Some(3), Some("main")),
            (Some(1), Some("develop")),
            (None, Some("main")),
        ] {
            let error = register(
                &config,
                &request("example-org/example-repo", worktrees, detach),
            )
            .expect_err("a different target configuration is refused");
            assert_eq!(
                error.first_id(),
                Some(ErrorId::TargetConfigurationMismatch),
                "{worktrees:?} {detach:?} produced the wrong error"
            );
        }

        // 組み合わせとして成立しないoptionは、保存値と比べる前に拒否する。
        let error = register(&config, &request("example-org/example-repo", Some(3), None))
            .expect_err("two worktrees still need an explicit branch");
        assert_eq!(error.first_id(), Some(ErrorId::WorktreesRequireDetach));
        assert_eq!(
            fs::read_to_string(
                ProjectPaths::derive(
                    &config.base_path,
                    &ProjectId::parse("example-org/example-repo")
                        .unwrap()
                        .canonical()
                )
                .metadata_file()
            )
            .unwrap(),
            before
        );
    }

    #[test]
    fn a_rebuild_in_progress_sends_the_user_to_rebuild() {
        let (_dir, config) = setup();
        let registration =
            register(&config, &request("example-org/example-repo", None, None)).expect("register");
        let paths = registration.paths.clone();
        let mut metadata = registration.metadata.clone();
        drop(registration);

        metadata.rebuild = Some(RebuildIntent {
            target_dockerfile_sha256: sha256_hex(b"target"),
            previous_dockerfile_sha256: metadata.provisioning.dockerfile_sha256.clone(),
        });
        metadata::update(&paths, &metadata).expect("record the intent");

        let error = register(&config, &request("example-org/example-repo", None, None))
            .expect_err("add does not continue through a rebuild");
        assert_eq!(error.first_id(), Some(ErrorId::RebuildIntentPending));
        let diagnostic = &error.diagnostics()[0];
        assert_eq!(
            diagnostic.remediation.as_ref().map(|message| message.id),
            Some("remediation-run-rebuild")
        );
    }

    #[test]
    fn the_project_lock_is_held_for_the_whole_workflow() {
        let (_dir, config) = setup();
        let registration =
            register(&config, &request("example-org/example-repo", None, None)).expect("register");
        let lock_path = registration.paths.lock_file();
        assert_eq!(mode_of(&lock_path), PRIVATE_FILE_MODE);

        let error = paths::acquire_exclusive_lock(
            &lock_path,
            Duration::from_millis(100),
            PRIVATE_FILE_MODE,
            PathScope::ProjectPath,
        )
        .expect_err("a second run waits for the first");
        assert_eq!(error.first_id(), Some(ErrorId::LockTimeout));

        drop(registration);
        paths::acquire_exclusive_lock(
            &lock_path,
            LOCK_TIMEOUT,
            PRIVATE_FILE_MODE,
            PathScope::ProjectPath,
        )
        .expect("the lock is released when the workflow ends");
    }

    #[test]
    fn a_broken_project_anywhere_under_the_base_path_stops_registration() {
        let (dir, config) = setup();
        let broken = dir.path().join("broken-org").join("broken-repo.project");
        fs::create_dir_all(broken.join(".sbxm")).unwrap();
        fs::write(broken.join(".sbxm").join("project.toml"), "version = 2\n").unwrap();

        let error = register(&config, &request("example-org/example-repo", None, None))
            .expect_err("a listing that cannot be trusted stops the run");
        assert!(error.contains(ErrorId::MetadataUnknownVersion), "{error:?}");
        assert!(
            !dir.path().join("example-org").exists(),
            "nothing may be created while the listing is broken"
        );
    }

    #[test]
    fn an_existing_non_directory_in_the_way_is_refused() {
        let (dir, config) = setup();
        fs::write(dir.path().join("example-org"), b"not a directory").unwrap();

        let error = register(&config, &request("example-org/example-repo", None, None))
            .expect_err("an owner path that is a file is refused");
        assert_eq!(error.first_id(), Some(ErrorId::ProjectPathUnexpectedType));
        assert_eq!(
            fs::read_to_string(dir.path().join("example-org")).unwrap(),
            "not a directory"
        );
    }
}
