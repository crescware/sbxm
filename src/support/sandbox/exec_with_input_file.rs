use std::path::Path;

use crate::boundary::host::{
    CommandOutcome, CommandSpec, EnvPolicy, HostEnvironment, TimeoutClass,
};
use crate::diagnostics::Result;

use super::exec_arguments;

/// Sandbox内でcommandを実行し、hostの`input`のfileをそのstdinへつなぐ。
///
/// repositoryのbundleのように大きな入力を、memoryへ読み込まずに運ぶ。`sbx exec`は`-i`が
/// 無ければstdinを中へつながない。
pub fn exec_with_input_file(
    host: &dyn HostEnvironment,
    sandbox: &str,
    args: &[&str],
    input: &Path,
) -> Result<CommandOutcome> {
    let mut full = exec_arguments(sandbox, None, args);
    full.insert(1, "-i".to_string());
    let borrowed: Vec<&str> = full.iter().map(String::as_str).collect();
    let spec = CommandSpec::capture("sbx", &borrowed)
        .env(EnvPolicy::InheritWithoutSshAgent)
        .timeout(TimeoutClass::RepositoryTransfer)
        .with_input_file(input);
    host.run(&spec)
}
