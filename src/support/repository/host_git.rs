use std::path::Path;

use crate::boundary::host::{
    CommandOutcome, CommandSpec, EnvPolicy, HostEnvironment, TimeoutClass,
};
use crate::diagnostics::Result;

/// hostの`repository`を作業directoryとしてgitを実行する。
///
/// 利用者のrepositoryで走らせる。sbxmが書き換えてよいのは、sbxm自身の名前空間の
/// refだけとする。呼び出し元が設定したrepositoryの場所を引き継がず、`repository`が
/// repositoryでなければ、上のdirectoryのrepositoryを使わずに失敗する。
///
/// `input`があればstdinへ渡す。終了statusの読み方は呼び出し側が決める。gitは答えの
/// 一部を終了statusで返すため、ここでは失敗へ写さない。
pub fn host_git(
    host: &dyn HostEnvironment,
    repository: &Path,
    args: &[&str],
    input: Option<Vec<u8>>,
    timeout: TimeoutClass,
) -> Result<CommandOutcome> {
    let mut spec = CommandSpec::capture("git", args)
        .env(EnvPolicy::HostRepository)
        .timeout(timeout)
        .working_dir(repository);
    if let Some(input) = input {
        spec = spec.with_input(input);
    }
    host.run(&spec)
}
