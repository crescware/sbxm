use crate::testing::host::{FakeSbx, isolated_agent, registered_secret};

/// 再作成後の検証を通るSandbox。secretがあり、SSH Agentへ到達できない。
pub fn verified(host: FakeSbx, name: &str) -> FakeSbx {
    isolated_agent(registered_secret(host, name), name)
}
