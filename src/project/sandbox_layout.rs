use super::CanonicalProjectId;

/// Sandbox内のpath。
///
/// bare repositoryとmanaged worktreeは、Sandbox内の`agent` homeの下に、案件名から
/// 決定的に導出したpathで置く。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxLayout {
    repository: String,
}

impl SandboxLayout {
    pub fn new(id: &CanonicalProjectId) -> SandboxLayout {
        SandboxLayout {
            repository: id.repository().to_string(),
        }
    }

    /// `/home/agent/work/<repository-lower>`
    ///
    /// このdirectory自体はworktreeではない。
    pub fn bare_root(&self) -> String {
        format!("/home/agent/work/{}", self.repository)
    }

    /// `<bare-root>/.git`
    pub fn bare_git_dir(&self) -> String {
        format!("{}/.git", self.bare_root())
    }

    /// `<repository-lower>.tree-<index>`。metadataが持つmanaged worktreeの名前。
    pub fn worktree_name(&self, index: u32) -> String {
        format!("{}.tree-{index}", self.repository)
    }

    /// `<bare-root>/<repository-lower>.tree-<index>`
    pub fn worktree(&self, index: u32) -> String {
        format!("{}/{}", self.bare_root(), self.worktree_name(index))
    }

    /// 案件が持つmanaged worktreeの名前。
    pub fn worktree_names(&self, count: u32) -> Vec<String> {
        (0..count).map(|index| self.worktree_name(index)).collect()
    }
}
