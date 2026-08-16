use crate::testing::outcome::Checked;

use crate::project::SandboxLayout;
use crate::testing::host::{FakeSbx, isolated_agent, registered_secret};
use crate::testing::project::{Fixture, Registered};
use crate::testing::value::COMMIT;

/// 中まで見られる稼働中Sandbox。`worktrees`は`worktree list --porcelain -z`の答え。
pub fn looking_inside(
    fixture: &Fixture,
    project: &Registered,
    worktrees: &str,
) -> Checked<FakeSbx> {
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let name = project.sandbox.as_str();
    let bare_git_dir = layout.bare_git_dir();
    let managed = format!("{}/example-repo.tree-0", layout.bare_root());
    let scratch = format!("{}/agent-scratch", layout.bare_root());
    let host = FakeSbx::listing(&format!(
        r#"{{"sandboxes":[{}]}}"#,
        fixture.entry(project, "running")?
    ))
    .answering(
        &format!(
            "exec {} -- git --git-dir {} rev-parse --is-bare-repository",
            project.sandbox,
            layout.bare_git_dir()
        ),
        0,
        "true\n",
    )
    .answering(
        &format!(
            "exec {} -- git --git-dir {} worktree list --porcelain -z",
            project.sandbox,
            layout.bare_git_dir()
        ),
        0,
        worktrees,
    )
    .answering(
        &format!(
            "exec {} -- git -C {}/agent-scratch status --porcelain=v2 -z --untracked-files=all",
            project.sandbox,
            layout.bare_root()
        ),
        0,
        "1 .M N... 100644 100644 100644 abc abc file.txt\0",
    )
    // statusのRemote列は、worktreeの作業状態とは別にHEADとupstreamを読む。
    .answering(
        &format!("exec {name} -- git -C {managed} rev-parse HEAD"),
        0,
        &format!("{COMMIT}\n"),
    )
    .answering(
        &format!("exec {name} -- git -C {managed} symbolic-ref --quiet --short HEAD"),
        0,
        "main\n",
    )
    .answering(
        &format!(
            "exec {name} -- git -C {managed} rev-parse --symbolic-full-name @{{upstream}}"
        ),
        0,
        "refs/remotes/origin/main\n",
    )
    .answering(
        &format!("exec {name} -- git -C {scratch} rev-parse HEAD"),
        0,
        &format!("{COMMIT}\n"),
    )
    .answering(
        &format!("exec {name} -- git --git-dir {bare_git_dir} config --get remote.origin.url"),
        0,
        "https://github.com/example-org/example-repo.git\n",
    )
    .answering(
        &format!(
            "exec {name} -- git --git-dir {bare_git_dir} for-each-ref --format=%(refname)%09%(objectname) refs/remotes/origin/"
        ),
        0,
        &format!("refs/remotes/origin/main\t{COMMIT}\n"),
    )
    .answering(
        &format!("exec {name} -- git --git-dir {bare_git_dir} cat-file -e {COMMIT}"),
        0,
        "",
    )
    .answering(
        &format!(
            "exec {name} -- git --git-dir {bare_git_dir} for-each-ref --format=%(refname) --contains={COMMIT} refs/remotes/origin/"
        ),
        0,
        "refs/remotes/origin/main\n",
    );
    Ok(isolated_agent(
        registered_secret(host, project.sandbox.as_str()),
        project.sandbox.as_str(),
    ))
}
