use crate::boundary::host::{CommandSpec, EnvPolicy, TimeoutClass};

use super::exec_arguments;

/// 出力をcaptureする`sbx exec`の指定。
///
/// Sandboxの中へ`SSH Agent`を持ち込まないよう、`SSH_AUTH_SOCK`を外して起動する。
pub(super) fn exec_spec(
    sandbox: &str,
    user: Option<&str>,
    stdin: bool,
    args: &[&str],
    timeout: TimeoutClass,
) -> CommandSpec {
    let full = exec_arguments(sandbox, user, stdin, args);
    let borrowed: Vec<&str> = full.iter().map(String::as_str).collect();
    CommandSpec::capture("sbx", &borrowed)
        .env(EnvPolicy::InheritWithoutSshAgent)
        .timeout(timeout)
}
