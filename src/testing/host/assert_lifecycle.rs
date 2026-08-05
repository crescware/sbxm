use crate::testing::outcome::Checked;

use crate::command::{EnvPolicy, OutputPolicy, TimeoutClass};

use super::FakeSbx;

/// Sandboxのlifecycleを動かす指定であることを確かめる。
///
/// 外部toolの進捗は隠さず、SSH Agentを渡さず、lifecycleのtimeoutで実行する。
pub fn assert_lifecycle(host: &FakeSbx, needle: &str) -> Checked {
    let spec = host.spec(needle)?;
    assert_eq!(
        spec.output(),
        OutputPolicy::Relay,
        "{needle} shows the external tool's own progress"
    );
    assert_eq!(
        spec.env,
        EnvPolicy::InheritWithoutSshAgent,
        "{needle} does not hand the SSH Agent to the sandbox"
    );
    assert_eq!(
        spec.timeout,
        TimeoutClass::SandboxLifecycle,
        "{needle} runs under the lifecycle timeout"
    );
    Ok(())
}
