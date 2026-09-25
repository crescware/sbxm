use std::path::Path;

use crate::boundary::host::{CommandOutcome, HostEnvironment, TimeoutClass};
use crate::diagnostics::Result;

use super::exec_spec;

/// Sandbox内でcommandを実行し、hostの`input`のfileをそのstdinへつなぐ。
///
/// repositoryのbundleのように大きな入力を、memoryへ読み込まずに運ぶ。
pub fn exec_with_input_file(
    host: &dyn HostEnvironment,
    sandbox: &str,
    args: &[&str],
    input: &Path,
) -> Result<CommandOutcome> {
    let spec = exec_spec(sandbox, None, true, args, TimeoutClass::RepositoryTransfer)
        .with_input_file(input);
    host.run(&spec)
}
