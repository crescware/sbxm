/// 接続先と、接続前に見せる情報。
#[derive(Debug)]
pub struct Prepared {
    pub project: String,
    pub sandbox: String,
    /// 接続先のSSH host名。
    pub ssh_host: String,
    pub worktrees: Vec<String>,
}
