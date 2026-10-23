use std::os::unix::process::ExitStatusExt;
use std::process::ExitStatus;

use crate::boundary::host::CommandOutcome;
use crate::boundary::host::protocol::InsideRecord;
use crate::diagnostics::{Result, unparseable};

use super::{Inside, exec_arguments};

/// まとめた確認が並べた結果から答える、Sandboxの中。
///
/// 答えは、`sbx exec`が1件ずつ起動したときと同じ形で持つ。programは`sbx`、argvは
/// `exec <sandbox> -- <command>`であり、診断に現れる起動の形は変わらない。
#[derive(Debug, Clone)]
pub struct Recorded {
    answers: Vec<(Vec<String>, CommandOutcome)>,
}

impl Recorded {
    /// `commands`を順に走らせた結果`records`を、commandごとの答えにする。
    pub fn new(sandbox: &str, commands: &[Vec<String>], records: Vec<InsideRecord>) -> Recorded {
        let answers = commands
            .iter()
            .zip(records)
            .map(|(command, record)| {
                let borrowed: Vec<&str> = command.iter().map(String::as_str).collect();
                let stderr_lossy = std::str::from_utf8(&record.stderr).is_err();
                let outcome = CommandOutcome {
                    program: "sbx".to_string(),
                    args: exec_arguments(sandbox, None, false, &borrowed),
                    working_dir: None,
                    // waitpid由来の値と同じく、終了codeは上位byteに置く。
                    status: ExitStatus::from_raw(record.status << 8),
                    stdout: record.stdout,
                    stderr: record.stderr,
                    stderr_lossy,
                };
                (command.clone(), outcome)
            })
            .collect();
        Recorded { answers }
    }
}

impl Inside for Recorded {
    /// 並べた結果のうち、同じargvのもの。
    ///
    /// 並べていないcommandを訊かれたら、答えを作らず断る。確認に含めるcommandと観測が
    /// 読むcommandがずれたことを、観測できなかった事実として残す。
    fn exec(&self, args: &[&str]) -> Result<CommandOutcome> {
        self.answers
            .iter()
            .find(|(command, _)| command.iter().map(String::as_str).eq(args.iter().copied()))
            .map(|(_, outcome)| outcome.clone())
            .ok_or_else(|| {
                unparseable(
                    "sbx exec",
                    &format!("the inside check did not run: {}", args.join(" ")),
                )
            })
    }
}
