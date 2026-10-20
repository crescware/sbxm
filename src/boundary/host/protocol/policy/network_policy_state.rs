/// `sbx policy ls --json`が示すgovernanceの状態。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkPolicyState {
    Available,
    Blocked,
}
