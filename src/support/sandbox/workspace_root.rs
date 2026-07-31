/// 中立Workspaceのroot。
///
/// 共有されるdirectoryの下にあるため、rootとその下のworkspaceの両方を、現在の
/// 利用者だけが使えるdirectoryとして検証または作成する。
pub const WORKSPACE_ROOT: &str = "/tmp/docker-sandboxes";
