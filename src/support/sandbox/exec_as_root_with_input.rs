use crate::boundary::host::{
    CommandOutcome, CommandSpec, EnvPolicy, HostEnvironment, TimeoutClass,
};
use crate::diagnostics::Result;

use super::exec_arguments;

/// Sandbox内でrootとしてcommandを実行し、`input`をそのstdinへ渡す。
///
/// hostからSandboxへbyte列を運ぶ経路はこれだけとする。通信はhostから始め、Sandboxへ
/// hostへの経路を与えない。`sbx exec`は`-i`が無ければstdinを中へつながない。
pub fn exec_as_root_with_input(
    host: &dyn HostEnvironment,
    sandbox: &str,
    args: &[&str],
    input: Vec<u8>,
) -> Result<CommandOutcome> {
    let mut full = exec_arguments(sandbox, Some("root"), args);
    full.insert(1, "-i".to_string());
    let borrowed: Vec<&str> = full.iter().map(String::as_str).collect();
    let spec = CommandSpec::capture("sbx", &borrowed)
        .env(EnvPolicy::InheritWithoutSshAgent)
        .timeout(TimeoutClass::SandboxLifecycle)
        .with_input(input);
    host.run(&spec)
}
