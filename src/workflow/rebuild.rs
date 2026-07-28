//! `sbxm rebuild`。
//!
//! 利用者が編集したDockerfileを新しい世代としてbuildし、保存されていない作業がない
//! ことを確かめてから、同じ目標構成でSandboxを作り直す。安全検査を省略するoptionは
//! 設けない。

use std::path::Path;

use crate::command::HostEnvironment;
use crate::compatibility::SandboxState;
use crate::config::GlobalConfig;
use crate::error::{Diagnostic, Error, ErrorId, Msg, Result};
use crate::metadata::{self, ProjectMetadata, RebuildIntent};
use crate::msg;
use crate::paths::{self, LOCK_TIMEOUT, PRIVATE_FILE_MODE, PathScope, ProjectPaths};
use crate::project::{ProjectId, SandboxLayout, SandboxName};

use super::files::{self, Conflict};
use super::image;
use super::inventory::{self, Poll, ProjectState};
use super::protection::{self, Unmanaged};
use super::{daemon, identity, repository, sandbox, secret, template};

/// `rebuild`の結果。
#[derive(Debug, Clone)]
pub struct RebuildOutput {
    pub project: String,
    pub sandbox: String,
    /// 適用済みになったDockerfile hash。
    pub applied: String,
    /// 何も変更しなかったか。
    pub unchanged: bool,
    pub warnings: Vec<Msg>,
}

/// Dockerfileの変更をSandboxへ適用する。
pub fn run(
    config: &GlobalConfig,
    project: &ProjectId,
    host: &dyn HostEnvironment,
    workspace_root: &Path,
    poll: Poll,
) -> Result<RebuildOutput> {
    let canonical = project.canonical();
    let paths = ProjectPaths::derive(&config.base_path, &canonical);
    let name = SandboxName::derive(&canonical);

    let Some(_) = metadata::load(&paths)? else {
        return Err(not_managed(project));
    };
    let _lock = paths::acquire_exclusive_lock(
        &paths.lock_file(),
        LOCK_TIMEOUT,
        PRIVATE_FILE_MODE,
        PathScope::ProjectPath,
    )?;

    // lock取得後の状態を正本とする。
    let mut project_metadata = metadata::load(&paths)?.ok_or_else(|| not_managed(project))?;
    let current = super::add::current_dockerfile_hash(&paths)?;
    // この案件のstateだけを、1回の一覧取得から決める。
    let entries = daemon::list(host)?;
    let state = inventory::state_of(&entries, &project_metadata, workspace_root)?;

    let target = match &project_metadata.rebuild {
        // intentがある場合は、intentに固定した世代だけを完成させる。
        Some(intent) => intent.target_dockerfile_sha256.clone(),
        None => {
            // 状態表が先にある。Sandboxを持たない案件には、変更の有無を答えない。
            require_created(&project_metadata, state, &name)?;
            if current == project_metadata.provisioning.dockerfile_sha256 {
                return Ok(RebuildOutput {
                    project: project_metadata.display_id(),
                    sandbox: name.as_str().to_string(),
                    applied: current,
                    unchanged: true,
                    warnings: Vec::new(),
                });
            }
            start_to_read_saved_state(
                host,
                &project_metadata,
                &name,
                state == ProjectState::Stopped,
                workspace_root,
                poll,
            )?;
            let layout = SandboxLayout::new(&canonical);
            protection::inspect(
                host,
                name.as_str(),
                &layout,
                &project_metadata,
                Unmanaged::Refused,
            )?;
            current.clone()
        }
    };

    // 新世代の成果物が揃うまで、既存Sandboxを停止も削除もしない。
    let built = prepare_generation(host, &paths, &name, &project_metadata, &target, &current)?;
    if project_metadata.rebuild.is_none() {
        project_metadata.rebuild = Some(RebuildIntent {
            target_dockerfile_sha256: target.clone(),
            previous_dockerfile_sha256: project_metadata.provisioning.dockerfile_sha256.clone(),
        });
        metadata::update(&paths, &project_metadata)?;
    }

    let mut warnings = built.warnings;
    if current != target {
        // intent記録後の編集は上書きせず、次の`rebuild`対象として案内する。
        warnings.push(msg!(
            "warning-dockerfile-changed-during-rebuild",
            project = project_metadata.display_id(),
            command = format!("sbxm rebuild {}", project_metadata.display_id())
        ));
    }

    let context = Switch {
        config,
        paths: &paths,
        project,
        workspace_root,
        poll,
    };
    context.run(host, &name, &mut project_metadata, &built.template)?;

    // 全検証が終わってから、適用済みhashを更新してintentを削除する。
    project_metadata.provisioning.dockerfile_sha256 = target.clone();
    project_metadata.rebuild = None;
    metadata::update(&paths, &project_metadata)?;

    Ok(RebuildOutput {
        project: project_metadata.display_id(),
        sandbox: name.as_str().to_string(),
        applied: target,
        unchanged: false,
        warnings,
    })
}

/// 新世代のimage、archive、Template。
struct Generation {
    template: super::template::LoadedTemplate,
    warnings: Vec<Msg>,
}

/// target世代の成果物を用意する。
///
/// 現在のDockerfileがtarget世代である場合だけ生成でき、異なる場合は既存の成果物が
/// 揃っていることを条件とする。世代を混在させない。
fn prepare_generation(
    host: &dyn HostEnvironment,
    paths: &ProjectPaths,
    name: &SandboxName,
    metadata: &ProjectMetadata,
    target: &str,
    current: &str,
) -> Result<Generation> {
    if current != target && !image::generation_is_built(host, name, &metadata.canonical_id, target)?
    {
        // 固定済みtargetの成果物がなく、Dockerfileも別世代であるため再生成できない。
        return Err(Error::single(
            Diagnostic::new(
                ErrorId::RebuildGenerationMissing,
                msg!(
                    "error-rebuild-generation-missing",
                    project = metadata.display_id(),
                    target = target,
                    observed = current
                ),
            )
            .remediation(msg!(
                "remediation-rebuild-generation-missing",
                command = format!("sbxm destroy --force {}", metadata.display_id())
            )),
        ));
    }

    let built = image::ensure(
        host,
        name,
        &metadata.canonical_id,
        &paths.dockerfile(),
        target,
    )?;
    // 中断した再構築を続ける場合、成功済みの工程はinspectしてskipする。
    let template = match template::existing(host, &built)? {
        Some(template) => template,
        None => {
            let archive = image::ensure_archive(host, paths, &built, target)?;
            template::ensure(host, &archive, &built)?
        }
    };
    Ok(Generation {
        template,
        warnings: built.warnings,
    })
}

/// Sandboxの切り替えが最初から最後まで使う文脈。
///
/// 工程ごとに変わるのはSandbox名、metadata、新Templateだけである。
struct Switch<'a> {
    config: &'a GlobalConfig,
    paths: &'a ProjectPaths,
    project: &'a ProjectId,
    workspace_root: &'a Path,
    poll: Poll,
}

impl Switch<'_> {
    /// Sandboxを新世代へ切り替える。
    fn run(
        &self,
        host: &dyn HostEnvironment,
        name: &SandboxName,
        metadata: &mut ProjectMetadata,
        template: &super::template::LoadedTemplate,
    ) -> Result<()> {
        let Switch {
            config,
            paths,
            project,
            workspace_root,
            poll,
        } = *self;
        let layout = SandboxLayout::new(&metadata.canonical_id);

        // 新世代の準備には時間がかかる。切り替える対象は、その後の観測から決める。
        let entries = daemon::list(host)?;
        // Sandboxが不在の中断点からは、作成工程から続ける。
        //
        // 既にあるSandboxがどちらの世代のものかは問わない。一覧はTemplateを示さず、
        // 世代を観測する手段がないためである。既存のSandboxは、保存されていない作業が
        // ないことを確かめてから必ず作り直す。
        if let Some(entry) = inventory::single(&entries, name.as_str())? {
            start_to_read_saved_state(
                host,
                metadata,
                name,
                entry.state == SandboxState::Stopped,
                workspace_root,
                poll,
            )?;
            protection::inspect(host, name.as_str(), &layout, metadata, Unmanaged::Refused)?;
            // データ保護検査は上で済ませている。
            inventory::remove(host, name, poll)?;
        }

        // 再作成したSandboxは、`prepare`と同じ条件でGitHubへ届く必要がある。custom secretは
        // 作成時に結び付くため、作り直す前に確認する。
        secret::require_github(host, name.as_str())?;

        let ready = sandbox::ensure(host, name, template, workspace_root)?;

        secret::require_placeholder_present(host, &ready.name)?;

        identity::ensure(host, &ready.name, &config.git)?;
        secret::configure_git_credential(host, &ready.name)?;
        files::place_all(host, &ready.name, &config.files, Conflict::Overwrite)?;
        repository::ensure_bare_clone(host, &ready.name, project, &layout)?;
        let branch = repository::resolve_start_ref(host, &ready.name, &layout, paths, metadata)?;
        repository::ensure_worktrees(host, &ready.name, &layout, paths, metadata, &branch)?;
        // 適用済みhashを更新する前に、credentialの隔離まで確かめる。
        sandbox::require_credentials_isolated(host, &ready.name)?;
        Ok(())
    }
}

/// 保存されていない作業を読むために、停止しているSandboxを起動する。
///
/// `rebuild`はこのSandboxをこれから作り直す。状態を読むためだけの起動を利用者へ
/// 求めない。
fn start_to_read_saved_state(
    host: &dyn HostEnvironment,
    metadata: &ProjectMetadata,
    name: &SandboxName,
    stopped: bool,
    workspace_root: &Path,
    poll: Poll,
) -> Result<()> {
    if !stopped {
        return Ok(());
    }
    inventory::start(host, name.as_str())?;
    inventory::wait_until_running(host, metadata, workspace_root, poll)?;
    Ok(())
}

/// `rebuild`は、Sandboxを持つ案件だけを対象とする。
fn require_created(
    metadata: &ProjectMetadata,
    state: ProjectState,
    name: &SandboxName,
) -> Result<()> {
    match state {
        ProjectState::Running | ProjectState::Stopped => Ok(()),
        ProjectState::NotCreated => Err(Error::single(
            Diagnostic::new(
                ErrorId::SandboxNotCreated,
                msg!(
                    "error-sandbox-not-created",
                    project = metadata.display_id(),
                    sandbox = name
                ),
            )
            .remediation(msg!(
                "remediation-sandbox-not-created",
                command = format!("sbxm add {}", metadata.display_id())
            )),
        )),
    }
}

fn not_managed(project: &ProjectId) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::ProjectNotManaged,
            msg!("error-project-not-managed", project = project),
        )
        .remediation(msg!(
            "remediation-project-not-managed",
            command = format!("sbxm add {project}")
        )),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{EnvPolicy, OutputPolicy, TimeoutClass};
    use crate::hash::sha256_hex;
    use crate::workflow::inventory::tests::{FakeSbx, fixture};
    use crate::workflow::protection::tests::clean_host;
    use std::os::unix::fs::PermissionsExt;
    use std::time::Duration;

    fn poll() -> Poll {
        Poll {
            interval: Duration::from_millis(1),
            limit: Duration::from_millis(20),
        }
    }

    fn project_id(value: &str) -> ProjectId {
        ProjectId::parse(value).expect("valid project id")
    }

    /// runtimeのimage storeが示す一覧。registry prefixを補って表示する。
    fn template_listing(image: &str) -> String {
        let (repository, tag) = image.rsplit_once(':').expect("an image reference");
        format!(
            r#"{{"images":[{{"id":"a3d0f4449170","repository":"docker.io/library/{repository}","tag":"{tag}"}}]}}"#
        )
    }
    /// 再作成後の検証を通るSandbox。secretがあり、SSH Agentへ到達できない。
    fn verified(host: FakeSbx, name: &str) -> FakeSbx {
        host.answering(
            &format!("secret ls {name}"),
            0,
            &format!(
                    "CUSTOM SECRETS\nSCOPE   TARGETS   ENV   PLACEHOLDER   SECRET\nx   {}   GH_TOKEN   sbx-cs-example   ghp_example\n",
                    crate::workflow::secret::GITHUB_HOSTS.join(" ")
                ),
        )
        .answering(&format!("exec {name} -- printenv SSH_AUTH_SOCK"), 1, "")
        .answering(&format!("exec {name} -- ssh-add -L"), 2, "")
        .answering(
            &format!("exec {name} -- sh -c {}", secret::placeholder_probe()),
            0,
            "sbx-cs-example",
        )
    }

    #[test]
    fn a_dockerfile_that_did_not_change_is_a_no_op() {
        let fixture = fixture();
        let mut project = fixture.register("example-org/example-repo");
        // 適用済みhashと同じ内容のDockerfileを置く。
        std::fs::write(project.paths.dockerfile(), "unchanged\n").unwrap();
        project.metadata.provisioning.dockerfile_sha256 = sha256_hex(b"unchanged\n");
        metadata::update(&project.paths, &project.metadata).unwrap();

        let host = FakeSbx::listing(&format!("[{}]", fixture.entry(&project, "running")));
        let output = run(
            &fixture.config,
            &project_id("example-org/example-repo"),
            &host,
            &fixture.workspace_root,
            poll(),
        )
        .expect("nothing to apply");

        assert!(output.unchanged);
        assert_eq!(output.applied, sha256_hex(b"unchanged\n"));
        assert!(
            !host.ran("build") && !host.ran("rm "),
            "a no-op touches nothing: {:?}",
            host.calls()
        );
    }

    #[test]
    fn a_project_whose_build_never_finished_is_sent_to_add_even_with_the_same_dockerfile() {
        let fixture = fixture();
        let mut project = fixture.register("example-org/example-repo");
        // `add`は登録時に適用済みhashを書く。Sandboxを作る前に中断した案件は、
        // 現在のDockerfileと同じhashを持ったまま`not-created`で残る。
        std::fs::write(project.paths.dockerfile(), "unchanged\n").unwrap();
        project.metadata.provisioning.dockerfile_sha256 = sha256_hex(b"unchanged\n");
        metadata::update(&project.paths, &project.metadata).unwrap();

        let host = FakeSbx::listing("[]");
        let error = run(
            &fixture.config,
            &project_id("example-org/example-repo"),
            &host,
            &fixture.workspace_root,
            poll(),
        )
        .expect_err("there is no sandbox to report as unchanged");
        assert_eq!(error.first_id(), Some(ErrorId::SandboxNotCreated));
    }

    #[test]
    fn a_project_that_is_not_managed_cannot_be_rebuilt() {
        let fixture = fixture();
        let host = FakeSbx::listing("[]");
        let error = run(
            &fixture.config,
            &project_id("example-org/example-repo"),
            &host,
            &fixture.workspace_root,
            poll(),
        )
        .expect_err("there is nothing to rebuild");
        assert_eq!(error.first_id(), Some(ErrorId::ProjectNotManaged));
    }

    #[test]
    fn a_stopped_sandbox_is_started_rather_than_handed_back_to_the_user() {
        // `rebuild`はこのSandboxをこれから作り直す。保存状態を読むためだけの起動を
        // 利用者へ求めない。
        let fixture = fixture();
        let project = fixture.register("example-org/example-repo");
        std::fs::write(project.paths.dockerfile(), "FROM scratch\n").unwrap();
        let name = project.sandbox.as_str();

        let stopped = format!("[{}]", fixture.entry(&project, "stopped"));
        let running = format!("[{}]", fixture.entry(&project, "running"));
        let host = FakeSbx::listings(&[&stopped, &running]);

        // 起動の先で止まってよい。ここで見たいのは、停止を理由に拒否しないことである。
        let _ = run(
            &fixture.config,
            &project_id("example-org/example-repo"),
            &host,
            &fixture.workspace_root,
            poll(),
        );

        assert!(
            host.ran(&format!("exec {name} -- /bin/true")),
            "the sandbox is started: {:?}",
            host.calls()
        );
    }

    #[test]
    fn a_project_without_a_sandbox_is_refused_with_the_command_that_helps() {
        let fixture = fixture();
        let project = fixture.register("example-org/example-repo");
        std::fs::write(project.paths.dockerfile(), "FROM scratch\n").unwrap();

        let absent = FakeSbx::listing("[]");
        let error = run(
            &fixture.config,
            &project_id("example-org/example-repo"),
            &absent,
            &fixture.workspace_root,
            poll(),
        )
        .expect_err("a project without a sandbox has nothing to switch");
        assert_eq!(error.first_id(), Some(ErrorId::SandboxNotCreated));
        assert!(!absent.ran("build"), "nothing is built");
    }

    #[test]
    fn unsaved_work_stops_the_rebuild_before_anything_is_built() {
        let fixture = fixture();
        let project = fixture.register("example-org/example-repo");
        std::fs::write(project.paths.dockerfile(), "FROM scratch\n").unwrap();
        let layout = SandboxLayout::new(&project.metadata.canonical_id);
        let name = project.sandbox.as_str();
        let managed = format!("{}/example-repo.tree-0", layout.bare_root());

        let host = clean_host(&fixture, &project).answering(
            &format!(
                "exec {name} -- git -C {managed} status --porcelain=v2 -z --untracked-files=all"
            ),
            0,
            "? scratch.txt\0",
        );

        let error = run(
            &fixture.config,
            &project_id("example-org/example-repo"),
            &host,
            &fixture.workspace_root,
            poll(),
        )
        .expect_err("a dirty worktree is not recreated");
        assert_eq!(error.first_id(), Some(ErrorId::UnsavedWork));
        assert!(
            !host.ran("build"),
            "the existing sandbox is untouched: {:?}",
            host.calls()
        );
    }

    #[test]
    fn the_sandbox_to_switch_is_decided_after_the_new_generation_is_ready() {
        let fixture = fixture();
        let project = fixture.register("example-org/example-repo");
        std::fs::write(project.paths.dockerfile(), "FROM scratch\n").unwrap();
        let target = sha256_hex(b"FROM scratch\n");
        let image = image::image_name(&project.sandbox, &target);
        let workspace = fixture.workspace_root.join(project.sandbox.as_str());
        std::fs::create_dir_all(&workspace).unwrap();
        std::fs::set_permissions(&workspace, std::fs::Permissions::from_mode(0o700)).unwrap();

        let host = clean_host(&fixture, &project)
            .answering(&format!("image ls --quiet {image}"), 0, "sha256:new\n")
            .answering(
                &format!("image inspect {image}"),
                0,
                &format!(
                    r#"[{{"Id":"sha256:new","Config":{{"Labels":{{"io.crescware.sbxm.canonical-id":"example-org/example-repo","io.crescware.sbxm.dockerfile-sha256":"{target}","io.crescware.sbxm.metadata-version":"1"}}}}}}]"#
                ),
            )
            .answering(
                "template ls --json",
                0,
                &template_listing(&image),
            );

        // 一覧は末尾から取り出される。世代の準備が終わるまでのあいだに、
        // 対象Sandboxが手作業で消された状況を作る。
        let running = format!("[{}]", fixture.entry(&project, "running"));
        let created = format!(
            r#"[{{"name":"{}","state":"running","workspace":"{}","template":"{image}","active_sessions":0}}]"#,
            project.sandbox,
            workspace.display()
        );
        // 一覧は末尾から取り出される。世代の準備が終わったあとの観測で、対象Sandboxが
        // 手作業で消えている状況になる。
        *host.listing.borrow_mut() = vec![created, "[]".to_string(), "[]".to_string(), running];

        let layout = SandboxLayout::new(&project.metadata.canonical_id);
        let git_dir = layout.bare_git_dir();
        let worktree = layout.worktree(0);
        let commit = "9f5b1c5a2b6d4e8f0a1b2c3d4e5f60718293a4b5";
        let name = project.sandbox.as_str();
        let host = verified(host, name)
            .answering(
                &format!("exec {name} -- git --git-dir {git_dir} rev-parse --is-bare-repository"),
                0,
                "true\n",
            )
            .answering(
                &format!(
                    "exec {name} -- git --git-dir {git_dir} config --get-all remote.origin.url"
                ),
                0,
                "https://github.com/example-org/example-repo.git\n",
            )
            .answering(
                &format!(
                    "exec {name} -- git --git-dir {git_dir} config --get-all remote.origin.fetch"
                ),
                0,
                "+refs/heads/*:refs/remotes/origin/*\n",
            )
            .answering(
                &format!(
                    "exec {name} -- git --git-dir {git_dir} rev-parse refs/remotes/origin/main"
                ),
                0,
                &format!("{commit}\n"),
            )
            .answering(
                &format!("exec {name} -- git -C {worktree} rev-parse HEAD"),
                0,
                &format!("{commit}\n"),
            )
            .answering(
                &format!("exec {name} -- git -C {worktree} symbolic-ref -q HEAD"),
                0,
                "refs/heads/main\n",
            );

        run(
            &fixture.config,
            &project_id("example-org/example-repo"),
            &host,
            &fixture.workspace_root,
            poll(),
        )
        .expect("the sandbox that is gone is created instead of removed");

        assert!(
            !host.ran("rm "),
            "a sandbox that no longer exists is not removed again: {:?}",
            host.calls()
        );
        assert!(
            host.ran("create --name"),
            "the run continued from the creation step: {:?}",
            host.calls()
        );
        // 外部toolの進捗は隠さず、SSH Agentを渡さず、lifecycleのtimeoutで実行する。
        let creation = host.spec("create --name");
        assert_eq!(creation.output, OutputPolicy::Passthrough);
        assert_eq!(creation.env, EnvPolicy::InheritWithoutSshAgent);
        assert_eq!(creation.timeout, TimeoutClass::SandboxLifecycle);
    }

    #[test]
    fn a_new_generation_that_cannot_be_produced_leaves_the_existing_sandbox_alone() {
        let fixture = fixture();
        let project = fixture.register("example-org/example-repo");
        std::fs::write(project.paths.dockerfile(), "FROM scratch\n").unwrap();
        // buildは走るが、そのあともimageは一覧に現れない。
        let host = clean_host(&fixture, &project);

        let error = run(
            &fixture.config,
            &project_id("example-org/example-repo"),
            &host,
            &fixture.workspace_root,
            poll(),
        )
        .expect_err("the new generation never became usable");
        assert_eq!(error.first_id(), Some(ErrorId::ImageUnusable));
        assert!(
            !host.ran("rm ") && !host.ran("create --name"),
            "the sandbox that is still running is untouched: {:?}",
            host.calls()
        );

        let stored = metadata::load(&project.paths).unwrap().expect("present");
        assert!(
            stored.rebuild.is_none(),
            "no generation was fixed, so there is nothing to continue"
        );
        assert_eq!(
            stored.provisioning.dockerfile_sha256, project.metadata.provisioning.dockerfile_sha256,
            "the applied generation did not move"
        );
    }

    #[test]
    fn a_fixed_generation_with_neither_artifacts_nor_its_dockerfile_says_how_to_recover() {
        let fixture = fixture();
        let project = fixture.register("example-org/example-repo");
        // Dockerfileは、固定した世代とは別の内容へ変わっている。
        std::fs::write(project.paths.dockerfile(), "FROM alpine\n").unwrap();

        let mut metadata = project.metadata.clone();
        metadata.rebuild = Some(RebuildIntent {
            target_dockerfile_sha256: sha256_hex(b"FROM scratch\n"),
            previous_dockerfile_sha256: metadata.provisioning.dockerfile_sha256.clone(),
        });
        metadata::update(&project.paths, &metadata).unwrap();

        let host = clean_host(&fixture, &project);
        let error = run(
            &fixture.config,
            &project_id("example-org/example-repo"),
            &host,
            &fixture.workspace_root,
            poll(),
        )
        .expect_err("generations are never mixed");
        assert_eq!(error.first_id(), Some(ErrorId::RebuildGenerationMissing));

        let diagnostic = &error.diagnostics()[0];
        assert_eq!(
            diagnostic.remediation.as_ref().map(|message| message.id),
            Some("remediation-rebuild-generation-missing")
        );
        assert!(
            !host.ran("build"),
            "the current Dockerfile is not built under the fixed generation's name: {:?}",
            host.calls()
        );
    }

    #[test]
    fn a_stopped_previous_generation_is_started_so_its_saved_state_can_be_read() {
        let fixture = fixture();
        let project = fixture.register("example-org/example-repo");
        std::fs::write(project.paths.dockerfile(), "FROM scratch\n").unwrap();
        let target = sha256_hex(b"FROM scratch\n");
        let previous = project.metadata.provisioning.dockerfile_sha256.clone();

        // Sandboxを削除する前に中断した状態。
        let mut metadata = project.metadata.clone();
        metadata.rebuild = Some(RebuildIntent {
            target_dockerfile_sha256: target.clone(),
            previous_dockerfile_sha256: previous.clone(),
        });
        metadata::update(&project.paths, &metadata).unwrap();

        let image = image::image_name(&project.sandbox, &target);
        let workspace = fixture.workspace_root.join(project.sandbox.as_str());
        std::fs::create_dir_all(&workspace).unwrap();
        std::fs::set_permissions(&workspace, std::fs::Permissions::from_mode(0o700)).unwrap();

        let stopped = format!("[{}]", fixture.entry(&project, "stopped"));
        let running = format!("[{}]", fixture.entry(&project, "running"));
        let created = format!(
            r#"[{{"name":"{}","state":"running","workspace":"{}","template":"{image}","active_sessions":0}}]"#,
            project.sandbox,
            workspace.display()
        );
        std::fs::set_permissions(&workspace, std::fs::Permissions::from_mode(0o700)).unwrap();

        let host = clean_host(&fixture, &project)
            .answering(&format!("image ls --quiet {image}"), 0, "sha256:new\n")
            .answering(
                &format!("image inspect {image}"),
                0,
                &format!(
                    r#"[{{"Id":"sha256:new","Config":{{"Labels":{{"io.crescware.sbxm.canonical-id":"example-org/example-repo","io.crescware.sbxm.dockerfile-sha256":"{target}","io.crescware.sbxm.metadata-version":"1"}}}}}}]"#
                ),
            )
            .answering(
                "template ls --json",
                0,
                &template_listing(&image),
            );

        let layout = SandboxLayout::new(&project.metadata.canonical_id);
        let git_dir = layout.bare_git_dir();
        let worktree = layout.worktree(0);
        let commit = "9f5b1c5a2b6d4e8f0a1b2c3d4e5f60718293a4b5";
        let name = project.sandbox.as_str();
        let host = verified(host, name)
            .answering(
                &format!("exec {name} -- git --git-dir {git_dir} rev-parse --is-bare-repository"),
                0,
                "true\n",
            )
            .answering(
                &format!(
                    "exec {name} -- git --git-dir {git_dir} config --get-all remote.origin.url"
                ),
                0,
                "https://github.com/example-org/example-repo.git\n",
            )
            .answering(
                &format!(
                    "exec {name} -- git --git-dir {git_dir} config --get-all remote.origin.fetch"
                ),
                0,
                "+refs/heads/*:refs/remotes/origin/*\n",
            )
            .answering(
                &format!(
                    "exec {name} -- git --git-dir {git_dir} rev-parse refs/remotes/origin/main"
                ),
                0,
                &format!("{commit}\n"),
            )
            .answering(
                &format!("exec {name} -- git -C {worktree} rev-parse HEAD"),
                0,
                &format!("{commit}\n"),
            )
            .answering(
                &format!("exec {name} -- git -C {worktree} symbolic-ref -q HEAD"),
                0,
                "refs/heads/main\n",
            );
        // 一覧は末尾から取り出される。停止中のprevious世代を起動し、検査してから消す。
        *host.listing.borrow_mut() = vec![
            created,
            "[]".to_string(),
            "[]".to_string(),
            running.clone(),
            running,
            stopped.clone(),
            stopped,
        ];

        run(
            &fixture.config,
            &project_id("example-org/example-repo"),
            &host,
            &fixture.workspace_root,
            poll(),
        )
        .expect("the fixed generation is completed from a stopped previous one");

        let calls = host.calls();
        let started = calls
            .iter()
            .position(|args| args.join(" ").contains("/bin/true"))
            .expect("the stopped sandbox is started before it is inspected");
        let removed = calls
            .iter()
            .position(|args| args.first().is_some_and(|arg| arg == "rm"))
            .expect("the previous generation is removed");
        assert!(
            started < removed,
            "the saved state is read from a running sandbox: {calls:?}"
        );
    }

    #[test]
    fn an_interrupted_rebuild_continues_from_the_generation_it_fixed() {
        let fixture = fixture();
        let project = fixture.register("example-org/example-repo");
        std::fs::write(project.paths.dockerfile(), "FROM scratch\n").unwrap();
        let target = sha256_hex(b"FROM scratch\n");

        // Sandbox削除の直後で中断した状態を作る。
        let mut metadata = project.metadata.clone();
        metadata.rebuild = Some(RebuildIntent {
            target_dockerfile_sha256: target.clone(),
            previous_dockerfile_sha256: metadata.provisioning.dockerfile_sha256.clone(),
        });
        metadata::update(&project.paths, &metadata).unwrap();

        let image = image::image_name(&project.sandbox, &target);
        let workspace = fixture.workspace_root.join(project.sandbox.as_str());
        std::fs::create_dir_all(&workspace).unwrap();
        std::fs::set_permissions(&workspace, std::fs::Permissions::from_mode(0o700)).unwrap();
        let created = format!(
            r#"[{{"name":"{}","state":"running","workspace":"{}","template":"{image}","active_sessions":0}}]"#,
            project.sandbox,
            workspace.display()
        );

        // 一覧は、run、Switch、作成前の確認、作成後の確認の順に読まれる。
        let host = FakeSbx::listings(&["[]", "[]", "[]", &created])
            // 固定した世代のimageは既にbuild済みである。
            .answering(&format!("image ls --quiet {image}"), 0, "sha256:new\n")
            .answering(
                &format!("image inspect {image}"),
                0,
                &format!(
                    r#"[{{"Id":"sha256:new","Config":{{"Labels":{{"io.crescware.sbxm.canonical-id":"example-org/example-repo","io.crescware.sbxm.dockerfile-sha256":"{target}","io.crescware.sbxm.metadata-version":"1"}}}}}}]"#
                ),
            )
            .answering(
                "template ls --json",
                0,
                &template_listing(&image),
            )
            .answering(
                &format!("secret ls {}", project.sandbox),
                0,
                &format!(
                    "CUSTOM SECRETS\nSCOPE   TARGETS   ENV   PLACEHOLDER   SECRET\nx   {}   GH_TOKEN   sbx-cs-example   ghp_example\n",
                    crate::workflow::secret::GITHUB_HOSTS.join(" ")
                ),
            );
        // 再作成後のSandbox内で、共有repositoryとworktreeが期待どおりに揃う。
        let layout = SandboxLayout::new(&project.metadata.canonical_id);
        let git_dir = layout.bare_git_dir();
        let worktree = layout.worktree(0);
        let commit = "9f5b1c5a2b6d4e8f0a1b2c3d4e5f60718293a4b5";
        let host = verified(host, project.sandbox.as_str())
            .answering(
                &format!(
                    "exec {} -- git --git-dir {git_dir} rev-parse --is-bare-repository",
                    project.sandbox
                ),
                0,
                "true\n",
            )
            .answering(
                &format!(
                    "exec {} -- git --git-dir {git_dir} config --get-all remote.origin.url",
                    project.sandbox
                ),
                0,
                "https://github.com/example-org/example-repo.git\n",
            )
            .answering(
                &format!(
                    "exec {} -- git --git-dir {git_dir} config --get-all remote.origin.fetch",
                    project.sandbox
                ),
                0,
                "+refs/heads/*:refs/remotes/origin/*\n",
            )
            .answering(
                &format!(
                    "exec {} -- git --git-dir {git_dir} rev-parse refs/remotes/origin/main",
                    project.sandbox
                ),
                0,
                &format!("{commit}\n"),
            )
            .answering(
                &format!(
                    "exec {} -- git -C {worktree} rev-parse HEAD",
                    project.sandbox
                ),
                0,
                &format!("{commit}\n"),
            )
            .answering(
                &format!(
                    "exec {} -- git -C {worktree} symbolic-ref -q HEAD",
                    project.sandbox
                ),
                0,
                "refs/heads/main\n",
            );

        let output = run(
            &fixture.config,
            &project_id("example-org/example-repo"),
            &host,
            &fixture.workspace_root,
            poll(),
        )
        .expect("the fixed generation is completed");

        assert_eq!(output.applied, target);
        assert!(!output.unchanged);
        let stored = metadata::load(&project.paths).unwrap().expect("present");
        assert_eq!(stored.provisioning.dockerfile_sha256, target);
        assert!(
            stored.rebuild.is_none(),
            "the intent is cleared once everything verified"
        );
        assert!(
            !host.ran("image save"),
            "an image that is already built is not rebuilt: {:?}",
            host.calls()
        );
        assert!(
            host.ran("secret ls") && host.ran("ssh-add -L"),
            "the recreated sandbox reaches GitHub and not the host agent: {:?}",
            host.calls()
        );

        // 判定に使う出力はsbxmが読む。
        assert_eq!(host.spec("ls --json").output, OutputPolicy::Capture);
        assert_eq!(
            host.spec("template ls --json").output,
            OutputPolicy::Capture
        );
    }
}
