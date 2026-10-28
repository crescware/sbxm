//! repositoryの開発用task。`cargo xtask <task>`で呼ぶ。
//!
//! 製品のcodeではないが、同じlintとtestの規律に従う。taskは1 task 1 moduleで持ち、本fileは
//! 引数をtaskへ引き渡して終了codeを返すだけを行う。

mod markdown;

use std::io::Write;
use std::process::ExitCode;

const USAGE: &str = "usage: cargo xtask markdown [--fix] [FILE...]";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut stdout = std::io::stdout().lock();
    let mut stderr = std::io::stderr().lock();
    let outcome = match args.split_first() {
        Some((task, rest)) if task == "markdown" => markdown::run(rest, &mut stdout, &mut stderr),
        _ => {
            // 伝える先がstderrしかないため、そこへ書けなかったことは捨てる。
            let _ = writeln!(stderr, "{USAGE}");
            return ExitCode::from(2);
        }
    };
    match outcome {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(error) => {
            let _ = writeln!(stderr, "xtask: {error}");
            ExitCode::FAILURE
        }
    }
}
