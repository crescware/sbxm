use std::cell::RefCell;
use std::io::Write;
use std::os::unix::process::ExitStatusExt;
use std::process::ExitStatus;
use std::rc::Rc;
use std::time::Duration;

use crate::design::{ExternalOutput, Trace};
use crate::diagnostics::{Error, Result};
use crate::time::Moment;

use super::protocol::{
    INSIDE_CHECK, inside_check_arguments, parse_inside_check, split_inside_check,
};
use super::{
    CommandOutcome, CommandSpec, Executable, HostEnvironment, OutputPolicy, TerminalCommand,
};

/// 外部commandの起動と所要時間を`Trace`へ並べながら、実行そのものは`inner`へ渡すhost。
///
/// `--verbose`の実行だけがこれを通る。実行の判断はいっさい変えず、起動の前後で時計を
/// 読む。argvは出すが、stdinへ渡したbyte列と、captureした出力の中身は出さない。前者は
/// 宣言fileの中身を含み、後者はbyte数で足りる。
///
/// Sandboxの中でまとめて走らせる確認は、手順に`-t`を渡してcommandごとの所要時間を
/// 計らせ、並んだ結果を`inside`の行として出す。計らせるのは所要時間を読む実行だけで
/// あり、読まない実行に、時刻を読む起動の代償を払わせない。
///
/// 端末そのものを渡す起動（SSH）は外部commandとして数えない。sessionの時間は利用者が
/// 持っていた時間である。渡す前に`Trace`がそこまでの合計を並べ、渡しているあいだは
/// 何も書かない。
pub struct Traced<'a> {
    inner: &'a dyn HostEnvironment,
    trace: Rc<RefCell<Trace>>,
}

/// 手順の本文の代わりに出す印。本文を出すと、1行が手順そのものの長さになる。
const SCRIPT_PLACEHOLDER: &str = "<inside_check.sh>";

impl<'a> Traced<'a> {
    pub fn new(inner: &'a dyn HostEnvironment, trace: Rc<RefCell<Trace>>) -> Traced<'a> {
        Traced { inner, trace }
    }

    /// 端末を`command`へ渡す`run`を、sessionとして並べる。
    fn session(
        &self,
        command: &TerminalCommand,
        run: impl FnOnce(&dyn HostEnvironment) -> Result<CommandOutcome>,
    ) -> Result<CommandOutcome> {
        let label = describe(command.spec());
        let started = self.trace.borrow_mut().hand_over(&label);
        let result = run(self.inner);
        let status = match &result {
            Ok(outcome) => outcome.status.to_string(),
            Err(error) => first_id(error),
        };
        self.trace
            .borrow_mut()
            .session_ended(&label, started, &status);
        result
    }

    fn begin(&self, spec: &CommandSpec) -> (String, Moment) {
        let command = describe(spec);
        let started = self.trace.borrow_mut().run(&command);
        (command, started)
    }

    /// captureした実行の終わり。
    fn end(
        &self,
        command: &str,
        started: Moment,
        result: &Result<CommandOutcome>,
        stdout: Option<usize>,
    ) {
        let mut trace = self.trace.borrow_mut();
        match result {
            Ok(outcome) => trace.done(
                command,
                started,
                &outcome.status.to_string(),
                stdout.unwrap_or(outcome.stdout.len()),
                outcome.stderr.len(),
            ),
            Err(error) => trace.failed(command, started, &first_id(error)),
        }
    }

    /// 端末へ出た実行の終わり。
    fn end_on_terminal(&self, command: &str, started: Moment, result: &Result<CommandOutcome>) {
        let mut trace = self.trace.borrow_mut();
        match result {
            Ok(outcome) => trace.done_on_terminal(command, started, &outcome.status.to_string()),
            Err(error) => trace.failed(command, started, &first_id(error)),
        }
    }

    /// まとめた確認が並べた、commandごとの結果。
    fn inside(&self, commands: &[Vec<String>], outcome: &CommandOutcome) {
        if !outcome.success() {
            return;
        }
        let Ok(records) = parse_inside_check(&outcome.stdout, commands.len()) else {
            return;
        };
        let mut trace = self.trace.borrow_mut();
        for (index, (command, record)) in commands.iter().zip(records).enumerate() {
            let status = ExitStatus::from_raw(record.status << 8).to_string();
            trace.inside(index, &words(command), &status, record.elapsed);
        }
    }
}

impl HostEnvironment for Traced<'_> {
    fn command_exists(&self, program: &str) -> bool {
        self.inner.command_exists(program)
    }

    /// 観測は外部commandを起動しないため、traceには並べない。
    fn inspect_executable(&self, program: &str) -> Option<Executable> {
        self.inner.inspect_executable(program)
    }

    fn run(&self, spec: &CommandSpec) -> Result<CommandOutcome> {
        let inside = inside_check(spec);
        let timed = inside
            .as_ref()
            .filter(|(timed, _)| !timed)
            .map(|(_, commands)| timed_inside_check(spec, commands));
        let spec = timed.as_ref().unwrap_or(spec);
        let (command, started) = self.begin(spec);
        let result = self.inner.run(spec);
        self.end(&command, started, &result, None);
        if let (Some((_, commands)), Ok(outcome)) = (&inside, &result) {
            self.inside(commands, outcome);
        }
        result
    }

    fn run_with_terminal(
        &self,
        command: &TerminalCommand,
        output: &mut dyn ExternalOutput,
    ) -> Result<CommandOutcome> {
        if hands_over(command) {
            return self.session(command, |inner| inner.run_with_terminal(command, output));
        }
        let (label, started) = self.begin(command.spec());
        let result = self.inner.run_with_terminal(command, output);
        self.end_on_terminal(&label, started, &result);
        result
    }

    fn run_with_terminal_ticking(
        &self,
        command: &TerminalCommand,
        output: &mut dyn ExternalOutput,
        every: Duration,
        tick: &mut dyn FnMut(),
    ) -> Result<CommandOutcome> {
        if hands_over(command) {
            return self.session(command, |inner| {
                inner.run_with_terminal_ticking(command, output, every, tick)
            });
        }
        let (label, started) = self.begin(command.spec());
        let result = self
            .inner
            .run_with_terminal_ticking(command, output, every, tick);
        self.end_on_terminal(&label, started, &result);
        result
    }

    fn run_streaming(
        &self,
        spec: &CommandSpec,
        sink: &mut dyn Write,
        limit: u64,
    ) -> Result<CommandOutcome> {
        let (command, started) = self.begin(spec);
        let mut counting = Counting {
            inner: sink,
            written: 0,
        };
        let result = self.inner.run_streaming(spec, &mut counting, limit);
        self.end(&command, started, &result, Some(counting.written));
        result
    }
}

/// `sink`へ流れたbyte数を数える。流したstdoutは溜めないため、長さは数えて知る。
struct Counting<'a> {
    inner: &'a mut dyn Write,
    written: usize,
}

impl Write for Counting<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let written = self.inner.write(bytes)?;
        self.written = self.written.saturating_add(written);
        Ok(written)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

/// 端末そのものを渡す起動か。
fn hands_over(command: &TerminalCommand) -> bool {
    command.spec().output() == OutputPolicy::HandOver
}

/// `spec`がまとめた確認なら、所要時間を計るかと、並べたcommand。
fn inside_check(spec: &CommandSpec) -> Option<(bool, Vec<Vec<String>>)> {
    if spec.program != "sbx" {
        return None;
    }
    let (_, timed, commands) = split_inside_check(&spec.args)?;
    Some((timed, commands))
}

/// `commands`を走らせる`spec`を、commandごとの所要時間も計る指定にする。
///
/// `--`より前の`exec`の引数はそのまま残し、手順とcommandの並びだけを作り直す。
fn timed_inside_check(spec: &CommandSpec, commands: &[Vec<String>]) -> CommandSpec {
    let mut args = exec_prefix(spec);
    args.extend(inside_check_arguments(commands, true));
    CommandSpec {
        args,
        ..spec.clone()
    }
}

/// `sbx exec`の引数のうち、`--`までの部分。まとめた確認だと分かった`spec`にだけ使う。
fn exec_prefix(spec: &CommandSpec) -> Vec<String> {
    let mut prefix: Vec<String> = spec
        .args
        .iter()
        .take_while(|arg| *arg != "--")
        .cloned()
        .collect();
    prefix.push("--".to_string());
    prefix
}

/// 起動を1行で示す。
///
/// まとめた確認は、手順の本文を印に、並べたcommandを件数に置き換える。commandは
/// `inside`の行が1件ずつ示す。
fn describe(spec: &CommandSpec) -> String {
    let mut parts = vec![quote(&spec.program)];
    if let Some((timed, commands)) = inside_check(spec) {
        parts.extend(exec_prefix(spec).iter().map(|arg| quote(arg)));
        parts.extend(["sh", "-c", SCRIPT_PLACEHOLDER, "sh"].map(String::from));
        if timed {
            parts.push("-t".to_string());
        }
        let plural = if commands.len() == 1 { "" } else { "s" };
        parts.push(format!("({} command{plural})", commands.len()));
        return parts.join(" ");
    }
    debug_assert!(!spec.args.iter().any(|arg| arg == INSIDE_CHECK));
    parts.extend(spec.args.iter().map(|arg| quote(arg)));
    parts.join(" ")
}

/// argvを空白で並べる。
fn words(argv: &[String]) -> String {
    argv.iter()
        .map(|arg| quote(arg))
        .collect::<Vec<String>>()
        .join(" ")
}

/// shellへ貼れる形で1語を示す。制御文字は見えるescapeへ変える。
fn quote(arg: &str) -> String {
    let visible: String = arg
        .chars()
        .flat_map(|character| {
            if character.is_control() {
                character.escape_default().collect::<Vec<char>>()
            } else {
                vec![character]
            }
        })
        .collect();
    let plain = !visible.is_empty()
        && visible
            .chars()
            .all(|character| character.is_alphanumeric() || "-_./:=@%+,".contains(character));
    if plain {
        visible
    } else {
        format!("'{}'", visible.replace('\'', "'\\''"))
    }
}

/// 失敗を名指す、翻訳しないID。診断を持たない失敗は中断である。
fn first_id(error: &Error) -> String {
    error.diagnostics().first().map_or_else(
        || "canceled".to_string(),
        |diagnostic| diagnostic.id.to_string(),
    )
}

#[cfg(test)]
#[path = "traced_test.rs"]
mod traced_test;
