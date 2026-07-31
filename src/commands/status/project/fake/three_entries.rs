use crate::project::SandboxLayout;
use crate::testing::project::Registered;

/// bare entryと、管理下・管理外のworktreeを1件ずつ並べたporcelain出力。
pub fn three_entries(project: &Registered) -> String {
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    format!(
        "worktree {root}\0bare\0\0worktree {root}/example-repo.tree-0\0branch refs/heads/main\0\0worktree {root}/agent-scratch\0detached\0\0",
        root = layout.bare_root()
    )
}
