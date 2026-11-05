use crate::boundary::host::HostEnvironment;
use crate::boundary::host::protocol::{InsideRecord, inside_check_arguments, parse_inside_check};
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
    // 70は手順がcommandを1つも実行する前に、一時directoryを作れなかったことを示す。
    // 満杯のSandboxにも接続・診断できるよう、同じ検査をhostのpipeとmemoryで読む。
    if outcome.status.code() == Some(70) {
        return without_temporary_files(host, sandbox, commands);
    }
    if !outcome.success() {
        return Err(unobservable(&outcome, SUBJECT));
    }
    let records = parse_inside_check(&outcome.stdout, commands.len())?;
    Ok(Recorded::new(sandbox, commands, records))
}

fn without_temporary_files(
    host: &dyn HostEnvironment,
    sandbox: &str,
    commands: &[Vec<String>],
) -> Result<Recorded> {
    let mut records = Vec::with_capacity(commands.len());
    for command in commands {
        let mut args: Vec<&str> = command.iter().map(String::as_str).collect();
        if args.first() == Some(&"git") {
            args.splice(0..0, ["env", "GIT_OPTIONAL_LOCKS=0", "GIT_NO_LAZY_FETCH=1"]);
        }
        let outcome = exec(host, sandbox, &args)?;
        let Some(status) = super::inner_exit_code(&outcome) else {
            return Err(unobservable(&outcome, SUBJECT));
        };
        records.push(InsideRecord {
            status,
            stdout: outcome.stdout,
            stderr: outcome.stderr,
            elapsed: None,
        });
    }
    Ok(Recorded::new(sandbox, commands, records))
}

#[cfg(test)]
#[path = "read_inside_test.rs"]
mod read_inside_test;
