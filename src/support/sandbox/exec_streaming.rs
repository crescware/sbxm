use std::io::Write;

use crate::boundary::host::{
    CommandOutcome, CommandSpec, EnvPolicy, HostEnvironment, TimeoutClass,
};
use crate::diagnostics::Result;

use super::exec_arguments;

/// Sandbox内でcommandを実行し、stdoutを`sink`へ流す。`limit`byteを超えたら拒否する。
///
/// Sandboxからhostへbyte列を運ぶ経路はこれだけとする。通信はhostから始め、Sandboxへ
/// hostへの経路を与えない。stdoutを溜めないため、大きな出力でもmemoryを使い切らない。
pub fn exec_streaming(
    host: &dyn HostEnvironment,
    sandbox: &str,
    args: &[&str],
    sink: &mut dyn Write,
    limit: u64,
    timeout: TimeoutClass,
) -> Result<CommandOutcome> {
    let full = exec_arguments(sandbox, None, args);
    let borrowed: Vec<&str> = full.iter().map(String::as_str).collect();
    let spec = CommandSpec::capture("sbx", &borrowed)
        .env(EnvPolicy::InheritWithoutSshAgent)
        .timeout(timeout);
    host.run_streaming(&spec, sink, limit)
}
