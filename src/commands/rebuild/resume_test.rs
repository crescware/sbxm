use super::super::fake::verified;
use super::*;
use crate::command::OutputPolicy;
use crate::hash::sha256_hex;
use crate::testing::host::FakeSbx;
use crate::testing::image::template_listing;
use crate::testing::poll::poll;
use crate::testing::project::{Fixture, Registered, fixture, project_id};
use crate::testing::value::COMMIT;
use std::os::unix::fs::PermissionsExt;

/// 固定した世代の成果物が揃い、再作成後の検証も通るhost。
///
/// 中断した`rebuild`の続きを、そのまま最後まで走らせられる状態を表す。
fn continuing(fixture: &Fixture, project: &Registered, target: &str) -> FakeSbx {
    let image = image::image_name(&project.sandbox, target);
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
            );
    // 再作成後のSandbox内で、共有repositoryとworktreeが期待どおりに揃う。
    let layout = SandboxLayout::new(&project.metadata.canonical_id);
    let git_dir = layout.bare_git_dir();
    let worktree = layout.worktree(0);
    verified(host, project.sandbox.as_str())
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
        )
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

    let host = continuing(&fixture, &project, &target);

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

#[test]
fn an_edit_made_after_the_generation_was_fixed_is_left_for_the_next_rebuild() {
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    let target = sha256_hex(b"FROM scratch\n");

    // 世代を固定したあとに、Dockerfileがさらに書き換えられた状態を作る。
    let mut metadata = project.metadata.clone();
    metadata.rebuild = Some(RebuildIntent {
        target_dockerfile_sha256: target.clone(),
        previous_dockerfile_sha256: metadata.provisioning.dockerfile_sha256.clone(),
    });
    metadata::update(&project.paths, &metadata).unwrap();
    std::fs::write(project.paths.dockerfile(), "FROM alpine\n").unwrap();

    let host = continuing(&fixture, &project, &target);
    let output = run(
        &fixture.config,
        &project_id("example-org/example-repo"),
        &host,
        &fixture.workspace_root,
        poll(),
    )
    .expect("the fixed generation is completed");

    assert_eq!(
        output.applied, target,
        "the generation that was fixed is the one that is applied"
    );
    assert_eq!(
        output
            .warnings
            .iter()
            .map(|message| message.id)
            .collect::<Vec<_>>(),
        vec!["warning-dockerfile-changed-during-rebuild"]
    );
    let stored = metadata::load(&project.paths).unwrap().expect("present");
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
}

#[test]
fn a_failure_after_the_switch_leaves_the_intent_in_place() {
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    std::fs::write(project.paths.dockerfile(), "FROM scratch\n").unwrap();
    let target = sha256_hex(b"FROM scratch\n");
    let previous = project.metadata.provisioning.dockerfile_sha256.clone();

    let mut metadata = project.metadata.clone();
    metadata.rebuild = Some(RebuildIntent {
        target_dockerfile_sha256: target.clone(),
        previous_dockerfile_sha256: previous.clone(),
    });
    metadata::update(&project.paths, &metadata).unwrap();

    // 切り替えの最後の検査だけを落とす。作り直したSandboxからhostのSSH Agentへ届く。
    let host = continuing(&fixture, &project, &target).answering(
        &format!("exec {} -- ssh-add -L", project.sandbox),
        0,
        "ssh-ed25519 AAAA example\n",
    );

    let error = run(
        &fixture.config,
        &project_id("example-org/example-repo"),
        &host,
        &fixture.workspace_root,
        poll(),
    )
    .expect_err("a sandbox that reaches the host agent is not accepted");
    assert_eq!(error.first_id(), Some(ErrorId::SshAgentExposed));

    let stored = metadata::load(&project.paths).unwrap().expect("present");
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
}
