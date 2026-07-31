/// environmentの扱い。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvPolicy {
    /// 現在processのenvironmentを継承する。
    Inherit,
    /// security-sensitiveな`sbx`起動。`SSH_AUTH_SOCK`を必ず除外する。
    InheritWithoutSshAgent,
}
