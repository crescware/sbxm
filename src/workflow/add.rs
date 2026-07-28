//! `sbxm add`。
//!
//! 新しいGitHub repositoryを管理対象へ登録し、構築が中断した案件には同じcommandで
//! 続きから進む。全工程を`inspect -> decide -> mutate -> verify -> record`で実行し、
//! 成功済みの成果物をrollback目的で削除しない。

use std::fs;
use std::path::{Path, PathBuf};

use crate::command::HostEnvironment;
use crate::compatibility::SandboxState;
use crate::config::{ConfigLocation, GlobalConfig};
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
use crate::project::{ProjectId, SandboxLayout, SandboxName};

use super::files::PlacedFile;
use super::{daemon, files, host_clone, identity, image, repository, sandbox, secret, template};

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
    /// 採用したDockerfileの現在のhash。metadataの適用済みhashとは別に持つ。
    pub dockerfile_sha256: String,
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
        Some(stored) => stored,
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
        dockerfile_sha256,
        _lock: lock,
    })
}

/// `add`の結果。成功出力の材料をそのまま持つ。
#[derive(Debug, Clone)]
pub struct AddOutput {
    pub project: String,
    pub sandbox: String,
    pub mode: CreationMode,
    pub start_ref: String,
    pub host_clone: PathBuf,
    pub sandbox_state: SandboxState,
    pub worktrees: Vec<WorktreeRow>,
    pub files: Vec<PlacedFile>,
    /// `mise`の設定を持つmanaged worktree。sbxmは自動実行せず案内だけを行う。
    pub mise_candidates: Vec<String>,
    /// 既に構築済みで、この実行が何も変更しなかったか。
    pub already_built: bool,
    pub warnings: Vec<Msg>,
}

/// 成功出力のworktree 1行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeRow {
    pub path: String,
    pub created_from: String,
    /// 観測できたHEAD。停止中のSandboxでは読めないため`None`になる。
    pub head: Option<String>,
    pub mode: CreationMode,
}

/// `mise`の設定として確認するfile。
const MISE_FILES: [&str; 3] = ["mise.toml", ".mise.toml", ".tool-versions"];

/// 案件を登録し、構築を最後まで進める。
///
/// 各工程は`inspect -> decide -> mutate -> verify -> record`で実行し、verifyに
/// 失敗したら後続工程へ進まない。成功済みの成果物はrollback目的で削除しない。
pub fn run(
    config: &GlobalConfig,
    location: &ConfigLocation,
    request: &AddRequest,
    host: &dyn HostEnvironment,
    workspace_root: &Path,
) -> Result<AddOutput> {
    let mut registration = register(config, request)?;
    let sandbox_name = registration.sandbox.clone();
    let layout = SandboxLayout::new(&registration.metadata.canonical_id);
    let mut warnings = Vec::new();

    if let Some(output) = already_built(host, &registration, &layout, workspace_root)? {
        return Ok(output);
    }

    let clone = host_clone::ensure(host, &registration.paths, &request.project)?;

    let generation = adopt_generation(host, &mut registration, &mut warnings)?;
    let image = image::ensure(
        host,
        &sandbox_name,
        &registration.metadata.canonical_id,
        &registration.paths.dockerfile(),
        &generation,
    )?;
    warnings.extend(image.warnings.clone());
    let archive = image::ensure_archive(host, &registration.paths, &image, &generation)?;
    let template = template::ensure(host, &archive, &image)?;

    // daemonを操作する区間は、project lockの後にglobal daemon lockを取得する。
    let daemon_guard = daemon::restart_without_ssh_agent(host, location)?;
    let ready = sandbox::ensure(host, &sandbox_name, &template, workspace_root)?;
    drop(daemon_guard);

    let files = files::place_all(host, &ready.name, &config.files, files::Conflict::Refuse)?;
    identity::ensure(host, &ready.name, &config.git)?;
    secret::require_github(host, &ready.name)?;

    repository::ensure_bare_clone(host, &ready.name, &request.project, &layout)?;
    let branch = repository::resolve_start_ref(
        host,
        &ready.name,
        &layout,
        &registration.paths,
        &mut registration.metadata,
    )?;
    let managed = repository::ensure_worktrees(
        host,
        &ready.name,
        &layout,
        &registration.paths,
        &mut registration.metadata,
        &branch,
    )?;

    let worktrees = observed_worktrees(host, &ready.name, &layout, &registration.metadata)?;
    let mise_candidates = mise_candidates(host, &ready.name, &layout, managed.len())?;

    Ok(AddOutput {
        project: registration.metadata.display_id(),
        sandbox: ready.name,
        mode: registration.metadata.provisioning.mode,
        start_ref: branch,
        host_clone: clone.path,
        sandbox_state: ready.state,
        worktrees,
        files,
        mise_candidates,
        already_built: false,
        warnings,
    })
}

/// 構築が完了している案件を、何も変更せずに報告する。
///
/// metadataが構築完了を宣言していることだけでは足りない。名前が一致するSandboxが、
/// この案件のworkspaceで、この案件のTemplateから動いていることを観測できた場合だけ
/// 構築済みとして報告し、確認できない場合は同じ工程と同じ診断で停止する。
fn already_built(
    host: &dyn HostEnvironment,
    registration: &Registration,
    layout: &SandboxLayout,
    workspace_root: &Path,
) -> Result<Option<AddOutput>> {
    let metadata = &registration.metadata;
    let provisioning = &metadata.provisioning;
    if provisioning.start_ref.is_none()
        || metadata.managed_worktrees.len() != provisioning.requested_worktrees as usize
    {
        return Ok(None);
    }

    let sandboxes = daemon::list(host)?;
    let Some(entry) = sandboxes
        .into_iter()
        .find(|entry| entry.name == registration.sandbox.as_str())
    else {
        return Ok(None);
    };

    // Templateは、metadataが正本とする世代から導出する。
    let templates = image::template_names(&registration.sandbox, metadata);
    sandbox::verify_identity(&entry, &registration.sandbox, &templates, workspace_root)?;

    let worktrees = observed_worktrees(host, &entry.name, layout, metadata)?;
    Ok(Some(AddOutput {
        project: metadata.display_id(),
        sandbox: entry.name,
        mode: provisioning.mode,
        start_ref: provisioning.start_ref.clone().unwrap_or_default(),
        host_clone: registration.paths.host_clone(),
        sandbox_state: entry.state,
        worktrees,
        files: Vec::new(),
        mise_candidates: Vec::new(),
        already_built: true,
        warnings: Vec::new(),
    }))
}

/// 初回構築を完成させる世代を決める。
///
/// 対応imageのbuck前にDockerfileが変わった場合は、現在のDockerfileを目標とする。
/// 既にimageがある場合は保存済み世代で完成させ、現在の内容は`rebuild`へ案内する。
fn adopt_generation(
    host: &dyn HostEnvironment,
    registration: &mut Registration,
    warnings: &mut Vec<Msg>,
) -> Result<String> {
    let stored = registration.metadata.provisioning.dockerfile_sha256.clone();
    let current = registration.dockerfile_sha256.clone();
    if current == stored {
        return Ok(stored);
    }

    if image::generation_is_built(
        host,
        &registration.sandbox,
        &registration.metadata.canonical_id,
        &stored,
    )? {
        // 初回構築の途中へ別世代を混在させない。
        warnings.push(msg!(
            "warning-dockerfile-changed-during-build",
            project = registration.metadata.display_id(),
            command = format!("sbxm rebuild {}", registration.metadata.display_id())
        ));
        return Ok(stored);
    }

    registration.metadata.provisioning.dockerfile_sha256 = current.clone();
    metadata::update(&registration.paths, &registration.metadata)?;
    Ok(current)
}

/// metadataが宣言するmanaged worktreeの現在の状態。
fn observed_worktrees(
    host: &dyn HostEnvironment,
    sandbox: &str,
    layout: &SandboxLayout,
    metadata: &ProjectMetadata,
) -> Result<Vec<WorktreeRow>> {
    let mode = metadata.provisioning.mode;
    let mut rows = Vec::with_capacity(metadata.managed_worktrees.len());
    for worktree in &metadata.managed_worktrees {
        let path = format!("{}/{}", layout.bare_root(), worktree.path);
        // 停止中のSandboxではHEADを読めない。観測できない値を推測で埋めない。
        let outcome = sandbox::exec(host, sandbox, &["git", "-C", &path, "rev-parse", "HEAD"])?;
        let head = outcome
            .success()
            .then(|| outcome.stdout_text().trim().to_string())
            .filter(|head| !head.is_empty());
        rows.push(WorktreeRow {
            path: worktree.path.clone(),
            created_from: worktree.created_from.clone(),
            head,
            mode,
        });
    }
    Ok(rows)
}

/// `mise`の設定を持つmanaged worktree。
fn mise_candidates(
    host: &dyn HostEnvironment,
    sandbox: &str,
    layout: &SandboxLayout,
    count: usize,
) -> Result<Vec<String>> {
    let mut candidates = Vec::new();
    for index in 0..count as u32 {
        let path = layout.worktree(index);
        for name in MISE_FILES {
            let target = format!("{path}/{name}");
            if sandbox::exec(host, sandbox, &["test", "-f", &target])?.success() {
                candidates.push(target);
            }
        }
    }
    Ok(candidates)
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
    if let Some(worktrees) = request.worktrees
        && provisioning.requested_worktrees != worktrees
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
mod tests {
    use super::*;
    use crate::command::{CommandOutcome, OutputPolicy, TimeoutClass};
    use crate::config::{GitIdentity, GlobalConfig};
    use crate::i18n::Locale;
    use crate::metadata::RebuildIntent;
    use crate::paths::AbsoluteBasePath;
    use crate::workflow::files::Placement;
    use std::cell::RefCell;
    use std::collections::{BTreeMap, BTreeSet};
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

    fn request(project: &str, worktrees: Option<u32>, detach: Option<&str>) -> AddRequest {
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

    const COMMIT: &str = "9f5b1c5a2b6d4e8f0a1b2c3d4e5f60718293a4b5";

    /// `sbx ls`だけを答え、Sandbox内のcommandは成功として扱うhost。
    struct FakeSbx {
        listing: String,
        calls: std::cell::RefCell<Vec<Vec<String>>>,
    }

    impl FakeSbx {
        fn listing(output: &str) -> FakeSbx {
            FakeSbx {
                listing: output.to_string(),
                calls: std::cell::RefCell::new(Vec::new()),
            }
        }

        fn ran(&self, needle: &str) -> bool {
            self.calls
                .borrow()
                .iter()
                .any(|args| args.join(" ").contains(needle))
        }
    }

    impl crate::command::HostEnvironment for FakeSbx {
        fn command_exists(&self, _program: &str) -> bool {
            true
        }

        fn run(
            &self,
            spec: &crate::command::CommandSpec,
        ) -> Result<crate::command::CommandOutcome> {
            use std::os::unix::process::ExitStatusExt;
            self.calls.borrow_mut().push(spec.args.clone());
            let stdout = if spec.args.first().is_some_and(|arg| arg == "ls") {
                self.listing.clone()
            } else if spec.args.iter().any(|arg| arg == "rev-parse") {
                format!("{COMMIT}\n")
            } else {
                String::new()
            };
            Ok(crate::command::CommandOutcome {
                program: spec.program.clone(),
                args: spec.args.clone(),
                working_dir: spec.working_dir.clone(),
                status: std::process::ExitStatus::from_raw(0),
                stdout: stdout.into_bytes(),
                stderr: Vec::new(),
                stderr_lossy: false,
            })
        }
    }

    /// 構築完了を宣言するmetadataへ書き換える。
    fn record_complete_build(registration: &Registration) {
        let mut metadata = registration.metadata.clone();
        metadata.provisioning.start_ref = Some("main".to_string());
        metadata.managed_worktrees = vec![crate::metadata::ManagedWorktree {
            path: "example-repo.tree-0".to_string(),
            created_from: crate::git::origin_ref("main"),
        }];
        metadata::update(&registration.paths, &metadata).expect("record the finished build");
    }

    fn sandbox_listing(name: &str, workspace: &Path, template: &str) -> String {
        format!(
            r#"[{{"name":"{name}","state":"running","workspace":"{}","template":"{template}","active_sessions":0}}]"#,
            workspace.display()
        )
    }

    #[test]
    fn a_finished_project_is_reported_without_changing_anything() {
        let (_dir, config) = setup();
        let home = tempfile::tempdir().unwrap();
        let location = ConfigLocation::from_home(home.path().to_path_buf());
        let workspace_root = tempfile::tempdir().unwrap();

        let registration =
            register(&config, &request("example-org/example-repo", None, None)).expect("register");
        record_complete_build(&registration);
        let sandbox = registration.sandbox.clone();
        let template = image::image_name(&sandbox, &registration.dockerfile_sha256);
        let paths = registration.paths.clone();
        drop(registration);

        let host = FakeSbx::listing(&sandbox_listing(
            sandbox.as_str(),
            &sandbox::workspace_path(workspace_root.path(), &sandbox),
            &template,
        ));
        let before = fs::read_to_string(paths.metadata_file()).unwrap();

        let output = run(
            &config,
            &location,
            &request("example-org/example-repo", None, None),
            &host,
            workspace_root.path(),
        )
        .expect("a finished project is a no-op success");

        assert!(output.already_built);
        assert_eq!(output.sandbox, sandbox.as_str());
        assert_eq!(output.worktrees.len(), 1);
        assert_eq!(output.worktrees[0].head.as_deref(), Some(COMMIT));
        for forbidden in ["create", "build", "image save", "template load", "clone"] {
            assert!(
                !host.ran(forbidden),
                "a finished project must not run {forbidden}: {:?}",
                host.calls.borrow()
            );
        }
        assert_eq!(
            fs::read_to_string(paths.metadata_file()).unwrap(),
            before,
            "the metadata of a finished project is left as it is"
        );
    }

    #[test]
    fn a_sandbox_that_cannot_be_identified_is_never_reported_as_finished() {
        let (_dir, config) = setup();
        let home = tempfile::tempdir().unwrap();
        let location = ConfigLocation::from_home(home.path().to_path_buf());
        let workspace_root = tempfile::tempdir().unwrap();

        let registration =
            register(&config, &request("example-org/example-repo", None, None)).expect("register");
        record_complete_build(&registration);
        let sandbox = registration.sandbox.clone();
        let template = image::image_name(&sandbox, &registration.dockerfile_sha256);
        let workspace = sandbox::workspace_path(workspace_root.path(), &sandbox);
        drop(registration);

        let elsewhere = workspace_root.path().join("elsewhere");
        fs::create_dir_all(&elsewhere).unwrap();
        let cases = [
            // 別のworkspaceで動いているSandbox。
            sandbox_listing(sandbox.as_str(), &elsewhere, &template),
            // 別のTemplateから作られたSandbox。
            sandbox_listing(
                sandbox.as_str(),
                &workspace,
                "sbxm-other-template:222222222222",
            ),
            // workspaceもTemplateも報告しないruntime。
            format!(r#"[{{"name":"{sandbox}","state":"running","active_sessions":0}}]"#),
        ];

        for listing in cases {
            let host = FakeSbx::listing(&listing);
            let error = run(
                &config,
                &location,
                &request("example-org/example-repo", None, None),
                &host,
                workspace_root.path(),
            )
            .expect_err("a sandbox that cannot be identified is not this project's");
            assert_eq!(
                error.first_id(),
                Some(ErrorId::SandboxUnusable),
                "listing {listing} produced the wrong error"
            );
        }
    }

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
            registration.dockerfile_sha256
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
            registration.dockerfile_sha256,
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

    /// docker、`sbx`、gitの応答を状態として持ち、`add`の全工程を通せるhost。
    ///
    /// 各工程の副作用は、その工程が成功したときにだけ起こす。中断した実行の続きを
    /// 同じ`add`が進められるかどうかは、この性質の上で判定できる。
    struct World {
        /// tag -> buildが宣言したlabel。
        images: RefCell<BTreeMap<String, Vec<(String, String)>>>,
        /// Template名 -> 対応するimage ID。
        templates: RefCell<BTreeMap<String, String>>,
        sandboxes: RefCell<Vec<SandboxRow>>,
        secrets: RefCell<Vec<String>>,
        /// Sandbox内に存在するpath。
        present: RefCell<BTreeSet<String>>,
        /// Sandbox内のfileのdigest。
        digests: RefCell<BTreeMap<String, String>>,
        /// Sandbox内のgitとghの設定。
        settings: RefCell<BTreeMap<String, String>>,
        /// bare repositoryの設定値。
        repository: RefCell<BTreeMap<String, String>>,
        /// managed worktreeのpath -> branch。detachedは`None`。
        worktrees: RefCell<BTreeMap<String, Option<String>>>,
        default_branch: String,
        /// 一致した起動を失敗させる。副作用は起こさない。
        fail: RefCell<Option<String>>,
        calls: RefCell<Vec<crate::command::CommandSpec>>,
    }

    #[derive(Clone)]
    struct SandboxRow {
        name: String,
        workspace: String,
        template: String,
    }

    const IMAGE_ID: &str =
        "sha256:3333333333333333333333333333333333333333333333333333333333333333";

    impl World {
        fn new() -> World {
            World {
                images: RefCell::new(BTreeMap::new()),
                templates: RefCell::new(BTreeMap::new()),
                sandboxes: RefCell::new(Vec::new()),
                secrets: RefCell::new(vec!["github".to_string()]),
                present: RefCell::new(BTreeSet::new()),
                digests: RefCell::new(BTreeMap::new()),
                settings: RefCell::new(BTreeMap::new()),
                repository: RefCell::new(BTreeMap::new()),
                worktrees: RefCell::new(BTreeMap::new()),
                default_branch: "main".to_string(),
                fail: RefCell::new(None),
                calls: RefCell::new(Vec::new()),
            }
        }

        /// 次の実行で、指定した起動だけを失敗させる。
        fn failing(&self, needle: &str) {
            *self.fail.borrow_mut() = Some(needle.to_string());
        }

        fn nothing_fails(&self) {
            *self.fail.borrow_mut() = None;
        }

        fn invocations(&self) -> Vec<String> {
            self.calls
                .borrow()
                .iter()
                .map(|spec| format!("{} {}", spec.program, spec.args.join(" ")))
                .collect()
        }

        fn ran(&self, needle: &str) -> bool {
            self.invocations().iter().any(|call| call.contains(needle))
        }

        /// ここまでの起動数。以降の起動だけを見るために使う。
        fn mark(&self) -> usize {
            self.calls.borrow().len()
        }

        fn since(&self, mark: usize) -> Vec<String> {
            self.invocations().split_off(mark)
        }

        fn policy_of(&self, needle: &str) -> Option<(crate::command::OutputPolicy, TimeoutClass)> {
            self.calls
                .borrow()
                .iter()
                .find(|spec| format!("{} {}", spec.program, spec.args.join(" ")).contains(needle))
                .map(|spec| (spec.output, spec.timeout))
        }

        fn outcome(
            &self,
            spec: &crate::command::CommandSpec,
            code: i32,
            stdout: &str,
        ) -> CommandOutcome {
            use std::os::unix::process::ExitStatusExt;
            CommandOutcome {
                program: spec.program.clone(),
                args: spec.args.clone(),
                working_dir: spec.working_dir.clone(),
                status: std::process::ExitStatus::from_raw(code << 8),
                stdout: stdout.as_bytes().to_vec(),
                stderr: Vec::new(),
                stderr_lossy: false,
            }
        }

        fn host_git(&self, spec: &crate::command::CommandSpec) -> (i32, String) {
            let args: Vec<&str> = spec.args.iter().map(String::as_str).collect();
            match args.as_slice() {
                ["clone", _, target] => {
                    // cloneが成功したときだけ、working treeができる。
                    fs::create_dir_all(Path::new(target).join(".git")).expect("create the clone");
                    (0, String::new())
                }
                ["rev-parse", "--is-bare-repository"] => (0, "false\n".to_string()),
                ["rev-parse", "--show-toplevel"] => (
                    0,
                    format!("{}\n", paths::display(spec.working_dir.as_ref().unwrap())),
                ),
                ["config", "--get-all", "remote.origin.url"] => (
                    0,
                    "git@github.com:Example-Org/Example-Repo.git\n".to_string(),
                ),
                _ => (0, String::new()),
            }
        }

        fn docker(&self, spec: &crate::command::CommandSpec) -> (i32, String) {
            let args: Vec<&str> = spec.args.iter().map(String::as_str).collect();
            match args.as_slice() {
                ["build", rest @ ..] => {
                    let mut labels = Vec::new();
                    let mut tag = String::new();
                    let mut index = 0;
                    while index < rest.len() {
                        match rest[index] {
                            "--label" => {
                                if let Some((key, value)) = rest[index + 1].split_once('=') {
                                    labels.push((key.to_string(), value.to_string()));
                                }
                                index += 2;
                            }
                            "--tag" => {
                                tag = rest[index + 1].to_string();
                                index += 2;
                            }
                            _ => index += 1,
                        }
                    }
                    self.images.borrow_mut().insert(tag, labels);
                    (0, String::new())
                }
                ["image", "ls", "--quiet", name] => (
                    0,
                    if self.images.borrow().contains_key(*name) {
                        "0123456789ab\n".to_string()
                    } else {
                        String::new()
                    },
                ),
                ["image", "inspect", name] => match self.images.borrow().get(*name) {
                    Some(labels) => {
                        let rendered = labels
                            .iter()
                            .map(|(key, value)| format!("\"{key}\":\"{value}\""))
                            .collect::<Vec<_>>()
                            .join(",");
                        (
                            0,
                            format!(
                                r#"[{{"Id":"{IMAGE_ID}","Config":{{"Labels":{{{rendered}}}}}}}]"#
                            ),
                        )
                    }
                    None => (1, String::new()),
                },
                ["image", "save", name, "--output", output] => {
                    // 実物と同じく、archiveはimage configをlabelごと持つ。
                    let labels = self.images.borrow().get(*name).cloned().unwrap_or_default();
                    let rendered = labels
                        .iter()
                        .map(|(key, value)| format!("\"{key}\":\"{value}\""))
                        .collect::<Vec<_>>()
                        .join(",");
                    let config = format!(r#"{{"config":{{"Labels":{{{rendered}}}}}}}"#);
                    let hex = IMAGE_ID.strip_prefix("sha256:").expect("a digest");
                    fs::write(
                        output,
                        crate::archive::tar_bytes(&[
                            (&format!("blobs/sha256/{hex}"), config.as_bytes()),
                            (
                                "manifest.json",
                                crate::archive::manifest_json(name, IMAGE_ID).as_bytes(),
                            ),
                        ]),
                    )
                    .expect("write the archive");
                    (0, String::new())
                }
                _ => (0, String::new()),
            }
        }

        fn sbx(&self, spec: &crate::command::CommandSpec) -> (i32, String) {
            let args: Vec<&str> = spec.args.iter().map(String::as_str).collect();
            match args.as_slice() {
                ["ls", "--json"] => {
                    let rendered = self
                    .sandboxes
                    .borrow()
                    .iter()
                    .map(|row| {
                        format!(
                            r#"{{"name":"{}","state":"running","workspace":"{}","template":"{}","active_sessions":0}}"#,
                            row.name, row.workspace, row.template
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(",");
                    (0, format!("[{rendered}]"))
                }
                ["template", "ls", "--json"] => {
                    // runtimeのimage storeはrepositoryとtagで示し、prefixを補う。
                    let rendered = self
                        .templates
                        .borrow()
                        .iter()
                        .map(|(name, id)| {
                            let (repository, tag) =
                                name.rsplit_once(':').expect("an image reference");
                            format!(
                                r#"{{"id":"{id}","repository":"docker.io/library/{repository}","tag":"{tag}"}}"#
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(",");
                    (0, format!(r#"{{"images":[{rendered}]}}"#))
                }
                ["template", "load", archive] => {
                    let manifest = crate::archive::read_manifest(Path::new(archive))
                        .expect("the archive names the image it holds");
                    self.templates.borrow_mut().insert(
                        manifest.repo_tags[0].clone(),
                        manifest.config_digest.clone(),
                    );
                    (0, String::new())
                }
                [
                    "create",
                    "--name",
                    name,
                    "--template",
                    template,
                    _kit,
                    workspace,
                ] => {
                    self.sandboxes.borrow_mut().push(SandboxRow {
                        name: name.to_string(),
                        workspace: workspace.to_string(),
                        template: template.to_string(),
                    });
                    (0, String::new())
                }
                ["secret", "ls", name] => {
                    let secrets = self.secrets.borrow();
                    if secrets.is_empty() {
                        return (0, format!("No secrets found for scope \"{name}\".\n"));
                    }
                    let mut table = String::from("SCOPE   TYPE      NAME     SECRET\n");
                    for secret in secrets.iter() {
                        table.push_str(&format!("{name}   service   {secret}   (stored)\n"));
                    }
                    (0, table)
                }
                ["cp", "--follow-link", source, target] => {
                    let digest = sha256_hex(&fs::read(source).expect("read the declared file"));
                    let path = target
                        .split_once(':')
                        .expect("a sandbox path")
                        .1
                        .to_string();
                    self.present.borrow_mut().insert(path.clone());
                    self.digests.borrow_mut().insert(path, digest);
                    (0, String::new())
                }
                ["daemon", ..] => (0, String::new()),
                _ => self.sandbox_exec(&args),
            }
        }

        /// `sbx exec [--user root] <name> -- <argv>`のargvを実行する。
        fn sandbox_exec(&self, args: &[&str]) -> (i32, String) {
            let Some(position) = args.iter().position(|arg| *arg == "--") else {
                return (0, String::new());
            };
            let inner = &args[position + 1..];
            let missing = (1, String::new());
            let ok = (0, String::new());

            match inner {
                ["test", flag, path] => {
                    let known = match *flag {
                        // 模したSandboxにsymlinkは存在しない。
                        "-h" => false,
                        _ => self.present.borrow().contains(*path),
                    };
                    if known { ok } else { missing }
                }
                ["mkdir", "-p", path] => {
                    self.present.borrow_mut().insert(path.to_string());
                    ok
                }
                ["sha256sum", path] => match self.digests.borrow().get(*path) {
                    Some(digest) => (0, format!("{digest}  {path}\n")),
                    None => missing,
                },
                ["install", "-d", .., path] => {
                    self.present.borrow_mut().insert(path.to_string());
                    ok
                }
                ["install", .., source, target] => {
                    let digest = self.digests.borrow().get(*source).cloned();
                    if let Some(digest) = digest {
                        self.present.borrow_mut().insert(target.to_string());
                        self.digests.borrow_mut().insert(target.to_string(), digest);
                    }
                    ok
                }
                ["mv", "-f", source, target] => {
                    let digest = self.digests.borrow_mut().remove(*source);
                    self.present.borrow_mut().remove(*source);
                    if let Some(digest) = digest {
                        self.present.borrow_mut().insert(target.to_string());
                        self.digests.borrow_mut().insert(target.to_string(), digest);
                    }
                    ok
                }
                ["rm", "-f", rest @ ..] => {
                    for path in rest {
                        self.present.borrow_mut().remove(*path);
                        self.digests.borrow_mut().remove(*path);
                    }
                    ok
                }
                ["git", "config", "--global", "--get", key] => {
                    match self.settings.borrow().get(*key) {
                        Some(value) => (0, format!("{value}\n")),
                        None => missing,
                    }
                }
                ["git", "config", "--global", key, value] => {
                    self.settings
                        .borrow_mut()
                        .insert(key.to_string(), value.to_string());
                    ok
                }
                ["gh", "config", "get", key, ..] => match self.settings.borrow().get(*key) {
                    Some(value) => (0, format!("{value}\n")),
                    None => missing,
                },
                ["gh", "config", "set", key, value, ..] => {
                    self.settings
                        .borrow_mut()
                        .insert(key.to_string(), value.to_string());
                    ok
                }
                ["git", "clone", "--bare", url, git_dir] => {
                    self.present.borrow_mut().insert(git_dir.to_string());
                    self.repository
                        .borrow_mut()
                        .insert("remote.origin.url".to_string(), url.to_string());
                    ok
                }
                ["git", "--git-dir", _, "config", "--get-all", key] => {
                    match self.repository.borrow().get(*key) {
                        Some(value) => (0, format!("{value}\n")),
                        None => missing,
                    }
                }
                ["git", "--git-dir", _, "config", key, value] => {
                    self.repository
                        .borrow_mut()
                        .insert(key.to_string(), value.to_string());
                    ok
                }
                ["git", "--git-dir", _, "rev-parse", "--is-bare-repository"] => {
                    (0, "true\n".to_string())
                }
                ["git", "--git-dir", _, "fsck", "--connectivity-only"] => ok,
                ["git", "--git-dir", _, "fetch", "--prune", "origin"] => ok,
                [
                    "git",
                    "--git-dir",
                    _,
                    "ls-remote",
                    "--symref",
                    "origin",
                    "HEAD",
                ] => (
                    0,
                    format!("ref: refs/heads/{}\tHEAD\n", self.default_branch),
                ),
                ["git", "check-ref-format", "--branch", _] => ok,
                [
                    "git",
                    "--git-dir",
                    _,
                    "show-ref",
                    "--verify",
                    "--quiet",
                    reference,
                ] => {
                    // 解決できないrefの扱いは、repository moduleのtestが固定する。
                    if reference.starts_with("refs/remotes/origin/") {
                        ok
                    } else {
                        missing
                    }
                }
                ["git", "--git-dir", _, "rev-parse", _] => (0, format!("{COMMIT}\n")),
                ["git", "--git-dir", _, "worktree", "add", rest @ ..] => {
                    let branch = rest
                        .iter()
                        .position(|arg| *arg == "-b")
                        .map(|index| rest[index + 1].to_string());
                    let path = rest
                        .iter()
                        .find(|arg| arg.contains(".tree-"))
                        .expect("a worktree path")
                        .to_string();
                    self.present.borrow_mut().insert(path.clone());
                    self.worktrees.borrow_mut().insert(path, branch);
                    ok
                }
                ["git", "-C", _, "rev-parse", "HEAD"] => (0, format!("{COMMIT}\n")),
                ["git", "-C", path, "symbolic-ref", "-q", "HEAD"] => {
                    match self.worktrees.borrow().get(*path) {
                        Some(Some(branch)) => (0, format!("refs/heads/{branch}\n")),
                        // detachedのworktreeはsymbolic refを持たない。
                        _ => missing,
                    }
                }
                _ => ok,
            }
        }
    }

    impl crate::command::HostEnvironment for World {
        fn command_exists(&self, _program: &str) -> bool {
            true
        }

        fn run(&self, spec: &crate::command::CommandSpec) -> Result<CommandOutcome> {
            self.calls.borrow_mut().push(spec.clone());
            let invocation = format!("{} {}", spec.program, spec.args.join(" "));
            if let Some(needle) = self.fail.borrow().as_deref()
                && invocation.contains(needle)
            {
                // 失敗した工程は、その工程の副作用を残さない。
                return Ok(self.outcome(spec, 1, ""));
            }

            let (code, stdout) = match spec.program.as_str() {
                "git" => self.host_git(spec),
                "docker" => self.docker(spec),
                "sbx" => self.sbx(spec),
                _ => (0, String::new()),
            };
            Ok(self.outcome(spec, code, &stdout))
        }
    }

    /// 宣言file 1件を持つ、実行時と同じ形の入力一式。
    struct Bench {
        _base: tempfile::TempDir,
        _home: tempfile::TempDir,
        workspace_root: tempfile::TempDir,
        config: GlobalConfig,
        location: ConfigLocation,
    }

    fn bench() -> Bench {
        let base = tempfile::tempdir().expect("temporary base path");
        let home = tempfile::tempdir().expect("temporary home");
        let workspace_root = tempfile::tempdir().expect("temporary workspace root");
        fs::set_permissions(
            workspace_root.path(),
            fs::Permissions::from_mode(PRIVATE_DIR_MODE),
        )
        .expect("the workspace root belongs to the current user only");

        let source = home.path().join("declared.toml");
        fs::write(&source, b"declared = true\n").expect("write the declared file");

        let config = GlobalConfig {
            language: Locale::En,
            base_path: AbsoluteBasePath::new(base.path()).expect("valid base path"),
            git: GitIdentity {
                user_name: "Example User".into(),
                user_email: "user@example.com".into(),
            },
            files: vec![crate::config::FileDeclaration {
                source: crate::config::HostFileSource::new(&paths::display(&source))
                    .expect("valid source"),
                destination: crate::config::SandboxHomeRelativePath::new(
                    ".config/example/config.toml",
                )
                .expect("valid destination"),
            }],
        };
        let location = ConfigLocation::from_home(home.path().to_path_buf());
        Bench {
            _base: base,
            _home: home,
            workspace_root,
            config,
            location,
        }
    }

    impl Bench {
        fn add(&self, world: &World, request: &AddRequest) -> Result<AddOutput> {
            run(
                &self.config,
                &self.location,
                request,
                world,
                self.workspace_root.path(),
            )
        }

        fn stored(&self, project: &str) -> ProjectMetadata {
            let canonical = ProjectId::parse(project).unwrap().canonical();
            let paths = ProjectPaths::derive(&self.config.base_path, &canonical);
            metadata::load(&paths)
                .expect("read the metadata")
                .expect("present")
        }
    }

    /// `add`が外部工程を呼ぶ順に並べた、失敗させる工程とその診断。
    const STEPS: [(&str, ErrorId); 12] = [
        ("git clone git@github.com", ErrorId::ExternalCommandFailed),
        ("docker build", ErrorId::ExternalCommandFailed),
        ("docker image save", ErrorId::ExternalCommandFailed),
        ("sbx template load", ErrorId::ExternalCommandFailed),
        ("sbx daemon stop", ErrorId::ExternalCommandFailed),
        ("sbx create", ErrorId::ExternalCommandFailed),
        ("sbx cp --follow-link", ErrorId::ExternalCommandFailed),
        ("config --global user.name", ErrorId::ExternalCommandFailed),
        ("sbx secret ls", ErrorId::ExternalCommandFailed),
        ("git clone --bare", ErrorId::ExternalCommandFailed),
        ("check-ref-format", ErrorId::InvalidBranchName),
        ("worktree add", ErrorId::ExternalCommandFailed),
    ];

    #[test]
    fn an_interruption_at_any_step_is_continued_by_the_same_add() {
        let bench = bench();
        let world = World::new();
        let request = request("Example-Org/Example-Repo", None, None);

        // 1工程ずつ後ろへずらして失敗させる。次の実行がそこまで進めることが継続の証拠になる。
        for (step, expected) in STEPS {
            world.failing(step);
            let error = bench
                .add(&world, &request)
                .expect_err("the run stops at the step that failed");
            assert_eq!(error.first_id(), Some(expected), "{step}");
            world.nothing_fails();
        }

        // 最後に失敗したのはworktree作成であり、続きの実行はそこから進む。
        let mark = world.mark();
        let output = bench.add(&world, &request).expect("the same add finishes");
        let tail = world.since(mark);

        assert!(!output.already_built);
        assert_eq!(output.mode, CreationMode::Attached);
        assert_eq!(output.start_ref, "main");
        assert_eq!(output.sandbox_state, SandboxState::Running);
        assert_eq!(output.worktrees.len(), 1);
        assert_eq!(output.worktrees[0].path, "example-repo.tree-0");
        assert_eq!(output.worktrees[0].head.as_deref(), Some(COMMIT));
        assert_eq!(output.files.len(), 1);
        assert_eq!(
            output.files[0].placement,
            Placement::Unchanged,
            "an earlier run placed the file, and an identical destination is left alone"
        );

        let stored = bench.stored("Example-Org/Example-Repo");
        assert_eq!(stored.provisioning.start_ref.as_deref(), Some("main"));
        assert_eq!(stored.managed_worktrees.len(), 1);

        // 成功済みの成果物は作り直さない。
        for done in [
            "git clone git@github.com",
            "docker build",
            "sbx template load",
            "sbx create",
            "git clone --bare",
        ] {
            assert!(
                !tail.iter().any(|call| call.contains(done)),
                "{done} was already done: {tail:?}"
            );
        }
        assert_eq!(
            tail.iter()
                .filter(|call| call.contains("worktree add"))
                .count(),
            1,
            "the run continues with the step that had failed: {tail:?}"
        );
        // archiveは工程へ到達するたびに作り直す。
        assert!(tail.iter().any(|call| call.contains("docker image save")));
    }

    #[test]
    fn a_finished_build_is_a_no_op_for_the_same_add() {
        let bench = bench();
        let world = World::new();
        let request = request("Example-Org/Example-Repo", None, None);
        bench.add(&world, &request).expect("the first run builds");

        let mark = world.mark();
        let output = bench
            .add(&world, &request)
            .expect("the second run changes nothing");

        assert!(output.already_built);
        for forbidden in [
            "docker build",
            "docker image save",
            "sbx template load",
            "sbx create",
            "sbx daemon stop",
            "sbx cp",
            "worktree add",
            "git clone",
        ] {
            assert!(
                !world
                    .since(mark)
                    .iter()
                    .any(|call| call.contains(forbidden)),
                "a finished project must not run {forbidden}: {:?}",
                world.since(mark)
            );
        }
    }

    #[test]
    fn a_missing_secret_stops_the_build_and_the_same_add_continues_once_it_is_there() {
        let bench = bench();
        let world = World::new();
        world.secrets.borrow_mut().clear();
        let request = request("Example-Org/Example-Repo", None, None);

        let error = bench
            .add(&world, &request)
            .expect_err("a build without repository access cannot continue");
        assert_eq!(error.first_id(), Some(ErrorId::GithubSecretMissing));
        assert!(
            !world.ran("git clone --bare"),
            "the sandbox repository is not cloned without the secret"
        );

        world.secrets.borrow_mut().push("github".to_string());
        let output = bench
            .add(&world, &request)
            .expect("the same add continues once the secret is registered");
        assert_eq!(output.worktrees.len(), 1);
        assert_eq!(
            world
                .invocations()
                .iter()
                .filter(|call| call.contains("sbx create"))
                .count(),
            1,
            "the sandbox that was already there is reused"
        );
    }

    #[test]
    fn three_detached_worktrees_start_from_one_commit_of_the_named_branch() {
        let bench = bench();
        let world = World::new();
        let request = request("Example-Org/Example-Repo", Some(3), Some("develop"));

        let output = bench.add(&world, &request).expect("build");
        assert_eq!(output.mode, CreationMode::Detached);
        assert_eq!(output.start_ref, "develop");
        assert_eq!(output.worktrees.len(), 3);
        for (index, worktree) in output.worktrees.iter().enumerate() {
            assert_eq!(worktree.path, format!("example-repo.tree-{index}"));
            assert_eq!(worktree.created_from, "refs/remotes/origin/develop");
            assert_eq!(worktree.head.as_deref(), Some(COMMIT));
            assert!(
                world.ran(&format!(
                    "worktree add --detach /home/agent/work/example-repo/example-repo.tree-{index} refs/remotes/origin/develop"
                )),
                "{:?}",
                world.invocations()
            );
        }
        // bare repositoryとworktreeは、1 treeでも3 treesでも分かれている。
        assert!(world.ran("git clone --bare https://github.com/Example-Org/Example-Repo.git /home/agent/work/example-repo/.git"));
    }

    #[test]
    fn the_long_steps_forward_their_progress_and_the_read_steps_are_captured() {
        let bench = bench();
        let world = World::new();
        bench
            .add(&world, &request("Example-Org/Example-Repo", None, None))
            .expect("build");

        for (needle, timeout) in [
            ("docker build", TimeoutClass::ImageBuild),
            ("docker image save", TimeoutClass::ImageBuild),
            ("git clone git@github.com", TimeoutClass::RepositoryTransfer),
            ("sbx template load", TimeoutClass::SandboxLifecycle),
            ("sbx create", TimeoutClass::SandboxLifecycle),
            ("git clone --bare", TimeoutClass::RepositoryTransfer),
            ("fetch --prune origin", TimeoutClass::RepositoryTransfer),
        ] {
            assert_eq!(
                world.policy_of(needle),
                Some((OutputPolicy::Passthrough, timeout)),
                "{needle} shows its progress while it runs"
            );
        }

        for needle in [
            "sbx ls --json",
            "docker image inspect",
            "sbx secret ls",
            "sbx template ls --json",
        ] {
            assert_eq!(
                world.policy_of(needle).map(|(output, _)| output),
                Some(OutputPolicy::Capture),
                "{needle} is read rather than shown"
            );
        }
    }

    #[test]
    fn the_declared_file_is_placed_once_and_left_alone_afterwards() {
        let bench = bench();
        let world = World::new();
        let request = request("Example-Org/Example-Repo", None, None);
        bench.add(&world, &request).expect("build");

        assert_eq!(
            world
                .digests
                .borrow()
                .get("/home/agent/.config/example/config.toml")
                .map(String::as_str),
            Some(sha256_hex(b"declared = true\n").as_str()),
            "the declared file reaches the destination it was declared for"
        );
        assert!(
            !world.present.borrow().contains("/tmp/sbxm-file-0"),
            "the staged copy does not survive the placement"
        );

        // 同じ内容の再配置は、Sandboxへ書き込まない。
        let world = World::new();
        world.digests.borrow_mut().insert(
            "/home/agent/.config/example/config.toml".to_string(),
            sha256_hex(b"declared = true\n"),
        );
        world
            .present
            .borrow_mut()
            .insert("/home/agent/.config/example/config.toml".to_string());
        let output = bench.add(&world, &request).expect("build");
        assert_eq!(output.files[0].placement, Placement::Unchanged);
        assert!(
            !world.ran("sbx cp"),
            "an identical destination is left alone"
        );
    }
}
