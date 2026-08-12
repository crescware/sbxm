/// rebuildとdestroyで、保護ゲートの判定が変わる点。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DestructiveOperation {
    /// 管理外の作業ツリーは`Blocker::UnmanagedWorktree`で拒否する。rebuildは同じ
    /// 配置を再作成できない。
    Rebuild,
    /// 管理外の作業ツリーは内容を他のworktreeと同列に検査し、存在自体は拒否しない。
    /// `WorktreeReport::kind`が管理外であることを示す。
    Destroy,
}
