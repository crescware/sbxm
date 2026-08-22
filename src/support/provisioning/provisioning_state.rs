/// 初回構築に関する、永続化せず毎回観測する状態。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProvisioningState {
    /// Sandboxも初回構築途中を示す可変成果物もない。
    Fresh,
    /// `InitialProvisioningIntent`が残っている。
    Pending,
    /// 目標構成を再観測できた。
    Ready,
    /// 何らかの初回構築成果物はあるが、完成を証明できない。
    Incomplete,
}
