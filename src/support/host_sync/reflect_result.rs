use std::path::PathBuf;

/// 保存したbranchやtagを、hostのbranchやtagへ反映した結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReflectResult {
    /// hostに無かったbranchやtagができた。
    Created,
    /// hostのbranchが、Sandboxの先端まで早送りで進んだ。checkoutしているbranchは、
    /// 作業treeごと進んだ。
    Updated,
    /// Sandboxのbranchが、hostのbranchより遅れているだけだった。hostは動かさない。
    Behind,
    /// hostとSandboxのbranchが、それぞれ相手に無いcommitを持っていた。
    Diverged,
    /// hostでcheckoutしているが、進められるworktreeが1つに決まらなかった。rebaseや
    /// bisectの途中でbranchを指していないworktreeや、directoryが無いworktreeである。
    CheckedOut,
    /// hostでcheckoutしているbranchを、そのworktreeの未commitの変更が重なるため進めな
    /// かった。
    LocalChanges {
        /// branchをcheckoutしているhostのworktree。
        worktree: PathBuf,
        /// Sandboxのcommitも変える、未commitの変更があるfile。worktreeからの相対path。
        paths: Vec<String>,
    },
    /// 同じ名前のtagが、hostで別の先を指していた。
    Exists,
    /// ほかの理由でhostのgitが断った。理由はgitの答えのまま持つ。
    Refused { reason: String },
}
