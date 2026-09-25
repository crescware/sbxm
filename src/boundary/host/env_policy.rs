/// environmentの扱い。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvPolicy {
    /// 現在processのenvironmentを継承する。
    Inherit,
    /// security-sensitiveな`sbx`起動。`SSH_AUTH_SOCK`だけを除外し、それ以外は
    /// 現在processのenvironmentをそのまま継承する。`DOCKER_SANDBOXES_ROOT_SIZE`の
    /// ような`sbx`自身が読むenvironment変数は、sbxmが解釈・変換せずこの経路で渡る。
    InheritWithoutSshAgent,
    /// hostにある利用者のrepositoryで走らせるgit。`SSH_AUTH_SOCK`を除き、呼び出し元が
    /// 設定したrepositoryの場所（`GIT_DIR`など）を引き継がない。作業directoryより上へ
    /// repositoryを探しに行かず、作業directoryがrepositoryでなければ失敗する。
    HostRepository,
}
