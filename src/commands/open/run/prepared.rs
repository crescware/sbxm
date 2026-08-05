/// 接続先と、接続前に見せる情報。
#[derive(Debug)]
pub struct Prepared {
    pub project: String,
    pub sandbox: String,
    /// 接続先のSSH host名。
    pub ssh_host: String,
    /// SSH sessionを開始するSandbox内のdirectory。
    pub working_directory: String,
    /// 指定されたindexが見つからず、repository rootへfallbackした場合のindex。
    pub missing_worktree_index: Option<u32>,
    pub worktrees: Vec<String>,
}
