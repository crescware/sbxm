use crate::project::SandboxLayout;

/// `list`が読むcommand。まとめた確認に含める。
pub fn list_command(layout: &SandboxLayout) -> Vec<String> {
    [
        "git",
        "--git-dir",
        &layout.bare_git_dir(),
        "worktree",
        "list",
        "--porcelain",
        "-z",
    ]
    .iter()
    .map(|arg| (*arg).to_string())
    .collect()
}
