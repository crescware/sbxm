/// environmentの扱い。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvPolicy {
    /// 現在processのenvironmentを継承する。
    Inherit,
    /// security-sensitiveな`sbx`起動。`SSH_AUTH_SOCK`だけを除外し、それ以外は
    /// 現在processのenvironmentをそのまま継承する。`DOCKER_SANDBOXES_ROOT_SIZE`の
    /// ような`sbx`自身が読むenvironment変数は、sbxmが解釈・変換せずこの経路で渡る。
    /// 通常の`sbx`起動には、利用状況の送信とfeature flagの取得を止める変数を付ける。
    InheritWithoutSshAgent,
    /// SSHの実効設定の観測と承認された修復。SSH agentと利用状況の送信を止めるが、
    /// feature flagの取得は呼び出し元と同じにする。無効化するとSSHの既定値自体が
    /// 変わり得るため、通常の`sbx`高速化とは分ける。
    SbxSsh,
    /// hostにある利用者のrepositoryで走らせるgit。`SSH_AUTH_SOCK`を除き、呼び出し元が
    /// 設定したrepositoryの場所（`GIT_DIR`など）と、gitが起動するsshの指定
    /// （`GIT_SSH_COMMAND`など）を引き継がない。作業directoryより上へrepositoryを
    /// 探しに行かず、作業directoryがrepositoryでなければ失敗する。
    HostRepository,
}
