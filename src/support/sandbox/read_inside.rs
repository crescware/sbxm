use crate::boundary::host::HostEnvironment;
use crate::boundary::host::protocol::{inside_check_arguments, parse_inside_check};
use crate::diagnostics::Result;

use super::{Recorded, exec, unobservable};

/// 診断に示す、まとめた確認が読むもの。翻訳しない。
const SUBJECT: &str = "SSH agent, credentials, identity, tools, files, repository, worktrees";

/// `commands`をSandboxの中で1回の`sbx exec`で走らせ、commandごとの答えを持ち帰る。
///
/// 手順そのものが答えなかった（起動できなかった、時間切れになった、出力を読めなかった）
/// 場合は`Err`であり、どのcommandの結果も無い。1件ずつ起動していたときと違い、1つの
/// commandの時間切れは全体の時間切れである。
pub fn read_inside(
    host: &dyn HostEnvironment,
    sandbox: &str,
    commands: &[Vec<String>],
) -> Result<Recorded> {
    // 所要時間は`--verbose`のhostが、この起動を見分けて計らせる。
    let args = inside_check_arguments(commands, false);
    let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
    let outcome = exec(host, sandbox, &borrowed)?;
    if !outcome.success() {
        return Err(unobservable(&outcome, SUBJECT));
    }
    let records = parse_inside_check(&outcome.stdout, commands.len())?;
    Ok(Recorded::new(sandbox, commands, records))
}
