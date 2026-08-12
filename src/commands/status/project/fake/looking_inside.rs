use crate::testing::outcome::Checked;

use crate::project::SandboxLayout;
use crate::testing::host::{FakeSbx, isolated_agent, registered_secret};
use crate::testing::project::{Fixture, Registered};

/// 中まで見られる稼働中Sandbox。`worktrees`は`worktree list --porcelain -z`の答え。
pub fn looking_inside(
    fixture: &Fixture,
    project: &Registered,
    worktrees: &str,
) -> Checked<FakeSbx> {
    let layout = SandboxLayout::new(project.metadata.canonical_id());
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
    );
    Ok(isolated_agent(
        registered_secret(host, project.sandbox.as_str()),
        project.sandbox.as_str(),
    ))
}
