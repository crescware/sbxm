use crate::diagnostics::ErrorId;
use crate::metadata::{self, RebuildIntent};
use crate::project::SandboxLayout;
use crate::support::image;

use crate::testing::outcome::{Checked, Refused, Required};

use super::{super::fake::verified, *};
use crate::command::{CommandOutcome, CommandSpec, HostEnvironment, OutputPolicy};
use crate::design::SilentProgress;
use crate::diagnostics::{Error, Result};
use crate::hash::{sha256_hex, short_hex};
use crate::msg;
use crate::testing::archive::image_archive_bytes;
use crate::testing::host::FakeSbx;
use crate::testing::image::template_listing;
use crate::testing::poll::poll;
use crate::testing::project::{Fixture, Registered, project_id};
use crate::testing::prompt::ScriptedPrompt;
use crate::testing::value::{COMMIT, IMAGE_ID};
use std::os::unix::fs::PermissionsExt;

/// 固定した世代の成果物が揃い、再作成後の検証も通るhost。
///
/// 中断した`rebuild`の続きを、そのまま最後まで走らせられる状態を表す。
fn continuing(fixture: &Fixture, project: &Registered, target: &str) -> Checked<FakeSbx> {
    let image = image::image_name(&project.sandbox, target);
    let workspace = fixture.workspace_root.join(project.sandbox.as_str());
    std::fs::create_dir_all(&workspace).required()?;
    std::fs::set_permissions(&workspace, std::fs::Permissions::from_mode(0o700)).required()?;
    let created = format!(
        r#"[{{"name":"{}","state":"running","workspace":"{}","template":"{image}","active_sessions":0}}]"#,
        project.sandbox,
        workspace.display()
    );

    // 一覧は、run、Switch、作成前の確認、作成後の確認の順に読まれる。
    let host = FakeSbx::listings(&["[]", "[]", "[]", &created])
            .answering("version --format {{.Server.Version}}", 0, "27.0.3\n")
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
                &template_listing(&image)?,
            );
    // 再作成後のSandbox内で、共有repositoryとworktreeが期待どおりに揃う。
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let git_dir = layout.bare_git_dir();
    let worktree = layout.worktree(0);
    Ok(verified(host, project.sandbox.as_str())
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
            &format!("{COMMIT}\n"),
        )
        .answering(
            &format!(
                "exec {} -- git -C {worktree} rev-parse HEAD",
                project.sandbox
            ),
            0,
            &format!("{COMMIT}\n"),
        )
        .answering(
            &format!(
                "exec {} -- git -C {worktree} rev-parse --path-format=absolute --git-common-dir",
                project.sandbox
            ),
            0,
            &format!("{git_dir}\n"),
        )
        .answering(
            &format!(
                "exec {} -- git -C {worktree} symbolic-ref -q HEAD",
                project.sandbox
            ),
            0,
            "refs/heads/main\n",
        ))
}

#[test]
fn an_interrupted_rebuild_continues_from_the_generation_it_fixed() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    std::fs::write(project.paths.dockerfile(), "FROM scratch\n").required()?;
    let target = sha256_hex(b"FROM scratch\n");

    // Sandbox削除の直後で中断した状態を作る。
    let mut metadata = project.metadata.clone();
    metadata.rebuild = Some(RebuildIntent {
        target_dockerfile_sha256: target.clone(),
        previous_dockerfile_sha256: metadata.provisioning.dockerfile_sha256.clone(),
    });
    metadata::update(&project.paths, &metadata).required()?;

    let host = continuing(&fixture, &project, &target)?;

    let output = run(
        Target {
            location: &fixture.location,
            requested: Some(&project_id("example-org/example-repo")?),
            prompt: &mut ScriptedPrompt::choosing(0),
        },
        &fixture.config,
        &host,
        &fixture.workspace_root,
        poll(),
        &mut SilentProgress,
    )
    .required_because("the fixed generation is completed")?;

    assert_eq!(output.applied, target);
    let stored = metadata::load(&project.paths)
        .required()?
        .required_because("present")?;
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
    assert_eq!(host.spec("ls --json")?.output(), OutputPolicy::Capture);
    assert_eq!(
        host.spec("template ls --json")?.output(),
        OutputPolicy::Capture
    );
    Ok(())
}

#[test]
fn an_edit_made_after_the_generation_was_fixed_is_left_for_the_next_rebuild() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let target = sha256_hex(b"FROM scratch\n");

    // 世代を固定したあとに、Dockerfileがさらに書き換えられた状態を作る。
    let mut metadata = project.metadata.clone();
    metadata.rebuild = Some(RebuildIntent {
        target_dockerfile_sha256: target.clone(),
        previous_dockerfile_sha256: metadata.provisioning.dockerfile_sha256.clone(),
    });
    metadata::update(&project.paths, &metadata).required()?;
    std::fs::write(project.paths.dockerfile(), "FROM alpine\n").required()?;

    let host = continuing(&fixture, &project, &target)?;
    let output = run(
        Target {
            location: &fixture.location,
            requested: Some(&project_id("example-org/example-repo")?),
            prompt: &mut ScriptedPrompt::choosing(0),
        },
        &fixture.config,
        &host,
        &fixture.workspace_root,
        poll(),
        &mut SilentProgress,
    )
    .required_because("the fixed generation is completed")?;

    assert_eq!(
        output.applied, target,
        "the generation that was fixed is the one that is applied"
    );
    assert_eq!(
        output
            .warnings
            .iter()
            .map(|warning| warning.description.id)
            .collect::<Vec<_>>(),
        vec!["warning-dockerfile-changed-during-rebuild"]
    );
    let stored = metadata::load(&project.paths)
        .required()?
        .required_because("present")?;
    assert_eq!(
        stored.provisioning.dockerfile_sha256, target,
        "the edit is not recorded as applied"
    );
    assert!(stored.rebuild.is_none());
    let edited = image::image_name(&project.sandbox, &sha256_hex(b"FROM alpine\n"));
    assert!(
        !host.ran(&edited),
        "the edit is left for the next rebuild: {:?}",
        host.calls()
    );
    Ok(())
}

#[test]
fn a_failure_after_the_switch_leaves_the_intent_in_place() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    std::fs::write(project.paths.dockerfile(), "FROM scratch\n").required()?;
    let target = sha256_hex(b"FROM scratch\n");
    let previous = project.metadata.provisioning.dockerfile_sha256.clone();

    let mut metadata = project.metadata.clone();
    metadata.rebuild = Some(RebuildIntent {
        target_dockerfile_sha256: target.clone(),
        previous_dockerfile_sha256: previous.clone(),
    });
    metadata::update(&project.paths, &metadata).required()?;

    // 切り替えの最後の検査だけを落とす。作り直したSandboxからhostのSSH Agentへ届く。
    let host = continuing(&fixture, &project, &target)?.answering(
        &format!("exec {} -- ssh-add -L", project.sandbox),
        0,
        "ssh-ed25519 AAAA example\n",
    );

    let error = run(
        Target {
            location: &fixture.location,
            requested: Some(&project_id("example-org/example-repo")?),
            prompt: &mut ScriptedPrompt::choosing(0),
        },
        &fixture.config,
        &host,
        &fixture.workspace_root,
        poll(),
        &mut SilentProgress,
    )
    .refused_because("a sandbox that reaches the host agent is not accepted")?;
    assert_eq!(error.first_id(), Some(ErrorId::SshAgentExposed));

    let stored = metadata::load(&project.paths)
        .required()?
        .required_because("present")?;
    assert_eq!(
        stored
            .rebuild
            .as_ref()
            .map(|intent| intent.target_dockerfile_sha256.as_str()),
        Some(target.as_str()),
        "the fixed generation is still there, so a re-run continues from it"
    );
    assert_eq!(
        stored.provisioning.dockerfile_sha256, previous,
        "the generation is not applied until every check has passed"
    );
    Ok(())
}

#[test]
fn a_stopped_sandbox_that_will_not_start_is_left_where_it_is() -> Checked {
    // 切り替えの直前でも、保存されていない作業は読んでから消す。読むための起動が
    // できなかった実行は、読めないまま削除へ進まない。
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    std::fs::write(project.paths.dockerfile(), "FROM scratch\n").required()?;
    let target = sha256_hex(b"FROM scratch\n");

    let mut metadata = project.metadata.clone();
    metadata.rebuild = Some(RebuildIntent {
        target_dockerfile_sha256: target.clone(),
        previous_dockerfile_sha256: metadata.provisioning.dockerfile_sha256.clone(),
    });
    metadata::update(&project.paths, &metadata).required()?;

    // 切り替える対象は停止しており、起動commandが通らない。
    let host = continuing(&fixture, &project, &target)?;
    *host.listing.borrow_mut() = vec![format!("[{}]", fixture.entry(&project, "stopped")?)];
    let host = host.answering(&format!("exec {} -- /bin/true", project.sandbox), 1, "");

    let error = run(
        Target {
            location: &fixture.location,
            requested: Some(&project_id("example-org/example-repo")?),
            prompt: &mut ScriptedPrompt::choosing(0),
        },
        &fixture.config,
        &host,
        &fixture.workspace_root,
        poll(),
        &mut SilentProgress,
    )
    .refused_because("the saved state cannot be read from a sandbox that will not start")?;

    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandFailed));
    assert!(
        !host.ran("rm "),
        "the sandbox is not removed before its state could be read: {:?}",
        host.calls()
    );
    let stored = metadata::load(&project.paths)
        .required()?
        .required_because("present")?;
    assert_eq!(
        stored
            .rebuild
            .as_ref()
            .map(|intent| intent.target_dockerfile_sha256.as_str()),
        Some(target.as_str()),
        "the fixed generation is still there, so a re-run continues from it"
    );
    Ok(())
}

/// `docker image save`が書くarchiveを、指定されたpathへ実際に置くhost。
///
/// 新世代のTemplateはarchiveをloadして作る。archiveが置かれるところまで通さないと、
/// その工程は動かせない。
struct SavingSbx {
    inner: FakeSbx,
    image: String,
    labels: Vec<(String, String)>,
}

impl HostEnvironment for SavingSbx {
    fn command_exists(&self, program: &str) -> bool {
        self.inner.command_exists(program)
    }

    fn run(&self, spec: &CommandSpec) -> Result<CommandOutcome> {
        let outcome = self.inner.run(spec)?;
        if spec.args.first().is_some_and(|arg| arg == "image")
            && spec.args.get(1).is_some_and(|arg| arg == "save")
            && let Some(output) = spec.args.iter().skip_while(|arg| *arg != "--output").nth(1)
        {
            let labels: Vec<(&str, &str)> = self
                .labels
                .iter()
                .map(|(key, value)| (key.as_str(), value.as_str()))
                .collect();
            std::fs::write(output, image_archive_bytes(&self.image, IMAGE_ID, &labels)).map_err(
                |error| {
                    Error::new(
                        ErrorId::ArchiveUnusable,
                        msg!("error-archive-unusable", detail = error),
                    )
                },
            )?;
        }
        Ok(outcome)
    }
}

#[test]
fn the_first_rebuild_of_a_generation_exports_its_archive_and_loads_the_template() -> Checked {
    // 世代ごとのTemplateは、imageを保存したarchiveからloadして作る。既にあるTemplateを
    // 使い回す経路とは別に、まだ無い世代を作る経路がある。
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    std::fs::write(project.paths.dockerfile(), "FROM scratch\n").required()?;
    // 登録済み案件は、世代別archiveの置き場を持っている。
    std::fs::create_dir_all(project.paths.cache_dir()).required()?;
    let target = sha256_hex(b"FROM scratch\n");

    let mut metadata = project.metadata.clone();
    metadata.rebuild = Some(RebuildIntent {
        target_dockerfile_sha256: target.clone(),
        previous_dockerfile_sha256: metadata.provisioning.dockerfile_sha256.clone(),
    });
    metadata::update(&project.paths, &metadata).required()?;

    let image = image::image_name(&project.sandbox, &target);
    // Templateはloadするまで一覧に現れない。
    let host = SavingSbx {
        inner: continuing(&fixture, &project, &target)?.answering_in_turn(
            "template ls --json",
            &[
                (0, r#"{"images":[]}"#),
                (0, r#"{"images":[]}"#),
                (0, &template_listing(&image)?),
            ],
        ),
        image: image.clone(),
        labels: image::expected_labels(project.metadata.canonical_id(), &target),
    };

    let output = run(
        Target {
            location: &fixture.location,
            requested: Some(&project_id("example-org/example-repo")?),
            prompt: &mut ScriptedPrompt::choosing(0),
        },
        &fixture.config,
        &host,
        &fixture.workspace_root,
        poll(),
        &mut SilentProgress,
    )
    .required_because("the generation is produced and applied")?;

    assert_eq!(output.applied, target);
    assert!(
        host.inner.ran("image save") && host.inner.ran("template load"),
        "the archive is exported and loaded: {:?}",
        host.inner.calls()
    );
    // 検証を終えたarchiveだけが正式な位置へ移る。
    let archive = project.paths.template_archive(short_hex(&target));
    assert!(archive.is_file(), "the archive of the generation is kept");
    assert!(
        !project
            .paths
            .template_archive_temp(short_hex(&target))
            .exists(),
        "the temporary archive does not survive the run"
    );

    let stored = metadata::load(&project.paths)
        .required()?
        .required_because("present")?;
    assert_eq!(stored.provisioning.dockerfile_sha256, target);
    assert!(stored.rebuild.is_none());
    Ok(())
}
