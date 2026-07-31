//! `status <project>`のtestが使う応答と、結果の読み取り。

use crate::commands::status::project::{ProjectStatus, Value};
use crate::project::SandboxLayout;
use crate::support::image;
use crate::testing::host::{FakeSbx, isolated_agent, registered_secret};
use crate::testing::project::{Fixture, Registered};

pub fn value_of(status: &ProjectStatus, item: &str) -> Value {
    status
        .items
        .iter()
        .find(|entry| entry.item == item)
        .unwrap_or_else(|| panic!("item {item} is missing"))
        .value
}

/// imageがまだ存在しないhost。一覧は答えるが、1件も返さない。
pub fn without_image(host: FakeSbx, project: &Registered) -> FakeSbx {
    let image = image::image_name(
        &project.sandbox,
        &project.metadata.provisioning.dockerfile_sha256,
    );
    host.answering(&format!("image ls --quiet {image}"), 0, "")
}

/// 中まで見られる稼働中Sandbox。`worktrees`は`worktree list --porcelain -z`の答え。
pub fn looking_inside(fixture: &Fixture, project: &Registered, worktrees: &str) -> FakeSbx {
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let host = FakeSbx::listing(&format!("[{}]", fixture.entry(project, "running")))
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
    isolated_agent(
        registered_secret(host, project.sandbox.as_str()),
        project.sandbox.as_str(),
    )
}

/// bare entryと、管理下・管理外のworktreeを1件ずつ並べたporcelain出力。
pub fn three_entries(project: &Registered) -> String {
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    format!(
        "worktree {root}\0bare\0\0worktree {root}/example-repo.tree-0\0branch refs/heads/main\0\0worktree {root}/agent-scratch\0detached\0\0",
        root = layout.bare_root()
    )
}
