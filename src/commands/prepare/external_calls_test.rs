//! `prepare`が外部へ触れる並びを、そのまま契約として固定する。
//!
//! 挙動不変を条件とする変更では、この並びが1行でも増減・前後すれば挙動が変わっている。
//! 期待値は実行して得た観測であり、書き換えることが「外部への触り方を変えた」という
//! 宣言になる。個別の工程を名指しして数えるテストは、名指ししなかった工程の同じ回帰を
//! 見逃すため、ここでは並びを丸ごと持つ。

use crate::design::SilentProgress;
use crate::diagnostics::ErrorId;
use crate::paths;
use crate::testing::add_request::{project_of, request};
use crate::testing::outcome::{Checked, Refused, Required};
use crate::testing::prompt::ScriptedPrompt;
use std::fs;

use super::{
    super::fake::{Bench, World},
    *,
};

/// 実行ごとに変わるpathだけを均す。commandの語順と語数はそのまま残す。
fn normalized(bench: &Bench, world: &World, mark: usize) -> Vec<String> {
    let parent = paths::display(bench.parent.as_path());
    let workspaces = paths::display(bench.workspace_root.path());
    let declared = paths::display(bench.config.files[0].source.as_path());
    world
        .invocations()
        .split_off(mark)
        .iter()
        .map(|call| {
            let call = call
                .replace(&parent, "<parent>")
                .replace(&workspaces, "<workspaces>")
                .replace(&declared, "<declared>");
            // build contextは実行ごとに名前が変わる一時directoryである。
            call.split(' ')
                .map(|word| match word.rsplit('/').next() {
                    Some(last) if last.starts_with("sbxm-build-context-") => "<context>",
                    _ => word,
                })
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect()
}

/// 期待と違う場合、そのまま貼り直せる形で実際の並びを示す。
fn assert_calls(label: &str, actual: &[String], expected: &[&str]) {
    let matched = actual.len() == expected.len()
        && actual.iter().zip(expected).all(|(got, want)| got == want);
    let rendered = actual
        .iter()
        .map(|call| format!("    {call:?},"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        matched,
        "{label}: 外部commandの並びが契約と違う（実際 {} 件 / 契約 {} 件）\n\
         実際の並び（このまま契約へ貼り直せる）:\n{rendered}\n",
        actual.len(),
        expected.len()
    );
}

/// `add`だけを済ませ、`prepare`の並びだけを見られる状態にする。
fn registered(bench: &Bench, world: &World, request: &crate::commands::add::AddRequest) -> Checked {
    crate::commands::add::run::run(
        &bench.location,
        &bench.parent,
        request,
        &crate::testing::metadata::git_identity(),
        world,
        &mut SilentProgress,
    )
    .required_because("the project is registered")?;
    Ok(())
}

fn prepare(
    bench: &Bench,
    world: &World,
    project: &crate::project::ProjectId,
) -> crate::diagnostics::Result<PrepareOutput> {
    run(
        &bench.location,
        &bench.config,
        Some(project),
        world,
        bench.workspace_root.path(),
        &mut ScriptedPrompt::choosing(0),
        &mut SilentProgress,
    )
}

const FRESH: &[&str] = &[
    "sbx ls --json",
    "sbx secret ls sbxm-example-org-example-repo-99a40327a69b",
    "docker version --format {{.Server.Version}}",
    "docker image ls --quiet sbxm-example-org-example-repo-99a40327a69b-template:4a0f8d41e27e",
    "docker build --label io.crescware.sbxm.canonical-id=example-org/example-repo --label io.crescware.sbxm.dockerfile-sha256=4a0f8d41e27e53198137451dd09bc8aa8b8704b1f879a77655d643302029e33a --label io.crescware.sbxm.metadata-version=1 --tag sbxm-example-org-example-repo-99a40327a69b-template:4a0f8d41e27e --file <parent>/example-repo.project/.sbxm/Dockerfile <context>",
    "docker image ls --quiet sbxm-example-org-example-repo-99a40327a69b-template:4a0f8d41e27e",
    "docker image inspect sbxm-example-org-example-repo-99a40327a69b-template:4a0f8d41e27e",
    "sbx template ls --json",
    "docker image save sbxm-example-org-example-repo-99a40327a69b-template:4a0f8d41e27e --output <parent>/example-repo.project/.sbxm/.cache/template-4a0f8d41e27e.tar.tmp",
    "sbx template ls --json",
    "sbx template load <parent>/example-repo.project/.sbxm/.cache/template-4a0f8d41e27e.tar",
    "sbx template ls --json",
    "sbx ls --json",
    "sbx create --name sbxm-example-org-example-repo-99a40327a69b --template sbxm-example-org-example-repo-99a40327a69b-template:4a0f8d41e27e shell <workspaces>/sbxm-example-org-example-repo-99a40327a69b",
    "sbx ls --json",
    "sbx exec sbxm-example-org-example-repo-99a40327a69b -- printenv SSH_AUTH_SOCK",
    "sbx exec sbxm-example-org-example-repo-99a40327a69b -- ssh-add -L",
    "sbx exec sbxm-example-org-example-repo-99a40327a69b -- sh -c printf %s \"${GH_TOKEN:-}\"",
    "sbx exec sbxm-example-org-example-repo-99a40327a69b -- test -h /home/agent/.config",
    "sbx exec sbxm-example-org-example-repo-99a40327a69b -- test -h /home/agent/.config/example",
    "sbx exec sbxm-example-org-example-repo-99a40327a69b -- test -h /home/agent/.config/example/settings.yaml",
    "sbx exec sbxm-example-org-example-repo-99a40327a69b -- test -e /home/agent/.config/example/settings.yaml",
    "sbx cp --follow-link <declared> sbxm-example-org-example-repo-99a40327a69b:/tmp/sbxm-file-0",
    "sbx exec --user root sbxm-example-org-example-repo-99a40327a69b -- install -d -o agent -g agent -m 0700 /home/agent/.config/example",
    "sbx exec --user root sbxm-example-org-example-repo-99a40327a69b -- install -o agent -g agent -m 0600 /tmp/sbxm-file-0 /home/agent/.config/example/settings.yaml.sbxm-new",
    "sbx exec --user root sbxm-example-org-example-repo-99a40327a69b -- mv -f /home/agent/.config/example/settings.yaml.sbxm-new /home/agent/.config/example/settings.yaml",
    "sbx exec --user root sbxm-example-org-example-repo-99a40327a69b -- rm -f /tmp/sbxm-file-0 /home/agent/.config/example/settings.yaml.sbxm-new",
    "sbx exec sbxm-example-org-example-repo-99a40327a69b -- git config --global --get user.name",
    "sbx exec sbxm-example-org-example-repo-99a40327a69b -- git config --global user.name Example User",
    "sbx exec sbxm-example-org-example-repo-99a40327a69b -- git config --global --get user.email",
    "sbx exec sbxm-example-org-example-repo-99a40327a69b -- git config --global user.email user@example.com",
    "sbx exec sbxm-example-org-example-repo-99a40327a69b -- sh -c for c in gh mise claude codex; do command -v \"$c\" > /dev/null 2>&1 && printf '%s\\n' \"$c\"; done",
    "sbx exec sbxm-example-org-example-repo-99a40327a69b -- gh config get git_protocol --host github.com",
    "sbx exec sbxm-example-org-example-repo-99a40327a69b -- gh config set git_protocol https --host github.com",
    "sbx exec sbxm-example-org-example-repo-99a40327a69b -- git config --global credential.https://github.com.helper !f() { echo username=x; echo password=$GH_TOKEN; }; f",
    "sbx exec sbxm-example-org-example-repo-99a40327a69b -- test -e /home/agent/work/example-repo/.git",
    "sbx exec sbxm-example-org-example-repo-99a40327a69b -- mkdir -p /home/agent/work/example-repo",
    "sbx exec sbxm-example-org-example-repo-99a40327a69b -- git init --bare /home/agent/work/example-repo/.git",
    "sbx exec sbxm-example-org-example-repo-99a40327a69b -- git --git-dir /home/agent/work/example-repo/.git remote add origin https://github.com/Example-Org/Example-Repo.git",
    "sbx exec sbxm-example-org-example-repo-99a40327a69b -- git --git-dir /home/agent/work/example-repo/.git config remote.origin.fetch +refs/heads/*:refs/remotes/origin/*",
    "sbx exec sbxm-example-org-example-repo-99a40327a69b -- git --git-dir /home/agent/work/example-repo/.git rev-parse --is-bare-repository",
    "sbx exec sbxm-example-org-example-repo-99a40327a69b -- git --git-dir /home/agent/work/example-repo/.git config --get-all remote.origin.url",
    "sbx exec sbxm-example-org-example-repo-99a40327a69b -- git --git-dir /home/agent/work/example-repo/.git config --get-all remote.origin.fetch",
    "sbx exec sbxm-example-org-example-repo-99a40327a69b -- git --git-dir /home/agent/work/example-repo/.git fsck --connectivity-only",
    "sbx exec sbxm-example-org-example-repo-99a40327a69b -- git --git-dir /home/agent/work/example-repo/.git fetch --prune --progress origin",
    "sbx exec sbxm-example-org-example-repo-99a40327a69b -- git --git-dir /home/agent/work/example-repo/.git ls-remote --symref origin HEAD",
    "sbx exec sbxm-example-org-example-repo-99a40327a69b -- git check-ref-format --branch main",
    "sbx exec sbxm-example-org-example-repo-99a40327a69b -- git --git-dir /home/agent/work/example-repo/.git show-ref --verify --quiet refs/remotes/origin/main",
    "sbx exec sbxm-example-org-example-repo-99a40327a69b -- git --git-dir /home/agent/work/example-repo/.git rev-parse refs/remotes/origin/main",
    "sbx exec sbxm-example-org-example-repo-99a40327a69b -- test -e /home/agent/work/example-repo/example-repo.tree-0",
    "sbx exec sbxm-example-org-example-repo-99a40327a69b -- test -e /home/agent/work/example-repo/example-repo.tree-0",
    "sbx exec sbxm-example-org-example-repo-99a40327a69b -- git --git-dir /home/agent/work/example-repo/.git worktree add --track -b main /home/agent/work/example-repo/example-repo.tree-0 refs/remotes/origin/main",
    "sbx exec sbxm-example-org-example-repo-99a40327a69b -- git -C /home/agent/work/example-repo/example-repo.tree-0 rev-parse HEAD",
    "sbx exec sbxm-example-org-example-repo-99a40327a69b -- git -C /home/agent/work/example-repo/example-repo.tree-0 symbolic-ref -q HEAD",
    "sbx exec sbxm-example-org-example-repo-99a40327a69b -- git -C /home/agent/work/example-repo/example-repo.tree-0 rev-parse HEAD",
];

#[test]
fn a_fresh_prepare_touches_the_outside_in_this_order() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    registered(&bench, &world, &request)?;

    let project = project_of(&request)?;
    let mark = world.mark();
    prepare(&bench, &world, &project).required_because("prepare succeeds")?;

    assert_calls("fresh", &normalized(&bench, &world, mark), FRESH);
    Ok(())
}

const READY_NO_OP: &[&str] = &[
    "sbx ls --json",
    "sbx exec sbxm-example-org-example-repo-99a40327a69b -- test -e /home/agent/work/example-repo/example-repo.tree-0",
    "sbx exec sbxm-example-org-example-repo-99a40327a69b -- git -C /home/agent/work/example-repo/example-repo.tree-0 rev-parse HEAD",
];

#[test]
fn a_ready_project_touches_the_outside_in_this_order() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    bench
        .build(&world, &request)
        .required_because("the first prepare succeeds")?;

    let project = project_of(&request)?;
    let mark = world.mark();
    let output = prepare(&bench, &world, &project).required_because("the second is a no-op")?;

    assert!(output.already_built);
    assert_calls("ready", &normalized(&bench, &world, mark), READY_NO_OP);
    Ok(())
}

/// prepareは中断した初回構築を継続しない。記録済みintentを見た時点でPendingとして
/// 拒否するため、`add`が撃つhost cloneの確認より先へは進まない。完成させるのは
/// `sbxm repair`である。
const INTERRUPTED: &[&str] = &[
    // `add`のhost clone確認。prepareはこの先へ進まない。
    "git rev-parse --is-bare-repository",
    "git rev-parse --show-toplevel",
    "git config --get-all remote.origin.url",
];

#[test]
fn a_prepare_after_an_interruption_touches_the_outside_in_this_order() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;

    world.failing("sbx create");
    bench
        .build(&world, &request)
        .refused_because("the run stops at the step that failed")?;
    world.nothing_fails();

    let mark = world.mark();
    let error = bench
        .build(&world, &request)
        .refused_because("an interrupted provisioning is finished only by repair")?;
    assert_eq!(error.first_id(), Some(ErrorId::InitialProvisioningPending));
    assert_calls(
        "interrupted",
        &normalized(&bench, &world, mark),
        INTERRUPTED,
    );
    Ok(())
}

/// build失敗後にDockerfileを直しても、通常のprepareは新しい世代へ移らない。記録済みの
/// target generationは`repair`だけが扱う。
const RETARGETED: &[&str] = &[
    // `add`のhost clone確認。prepareはこの先へ進まない。
    "git rev-parse --is-bare-repository",
    "git rev-parse --show-toplevel",
    "git config --get-all remote.origin.url",
];

#[test]
fn a_prepare_after_a_failed_build_and_a_fixed_dockerfile_touches_the_outside_in_this_order()
-> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;

    world.failing("docker build");
    bench
        .build(&world, &request)
        .refused_because("the image cannot be built")?;
    world.nothing_fails();

    let paths =
        crate::paths::ProjectPaths::derive(&bench.parent, request.repository.canonical_id());
    fs::write(paths.dockerfile(), b"FROM example:edited\n").required_because("fix it")?;

    let mark = world.mark();
    let error = bench
        .build(&world, &request)
        .refused_because("the recorded target generation is only repair's to move")?;
    assert_eq!(error.first_id(), Some(ErrorId::InitialProvisioningPending));

    assert_calls("retargeted", &normalized(&bench, &world, mark), RETARGETED);
    Ok(())
}

const COLLISION: &[&str] = &[
    "sbx ls --json",
    "sbx secret ls sbxm-example-org-example-repo-99a40327a69b",
    "docker version --format {{.Server.Version}}",
    "docker image ls --quiet sbxm-example-org-example-repo-99a40327a69b-template:4a0f8d41e27e",
    "docker image inspect sbxm-example-org-example-repo-99a40327a69b-template:4a0f8d41e27e",
];

#[test]
fn a_foreign_image_stops_prepare_after_exactly_these_calls() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    registered(&bench, &world, &request)?;

    let stored = bench.stored("Example-Org/Example-Repo")?;
    let name = crate::project::SandboxName::derive(request.repository.canonical_id());
    let image_name =
        crate::support::image::image_name(&name, &stored.provisioning.dockerfile_sha256);
    world.images.borrow_mut().insert(
        image_name,
        vec![(
            crate::support::image::LABEL_CANONICAL_ID.to_string(),
            "Other-Org/Other-Repo".to_string(),
        )],
    );

    let project = project_of(&request)?;
    let mark = world.mark();
    let error =
        prepare(&bench, &world, &project).refused_because("a foreign image is not overwritten")?;

    assert_eq!(error.first_id(), Some(ErrorId::ImageUnusable));
    assert_calls("collision", &normalized(&bench, &world, mark), COLLISION);
    Ok(())
}
