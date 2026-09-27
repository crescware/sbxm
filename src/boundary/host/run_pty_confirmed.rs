use std::io::{Read, Write};
use std::process::{Command, ExitStatus};

use rustix::io::Errno;

use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;
use crate::time::Clock;

use super::{
    CommandOutcome, Processes, Pty, PtyConfirmedCommand, WAIT_POLL_INTERVAL, apply_env,
    spawn_failure, start_child, terminate_child, unreadable,
};

/// 端末の大きさ。値を固定し、折り返しを実行環境から切り離す。
const ROWS: u16 = 24;
const COLUMNS: u16 = 120;

/// 確認promptにだけ答え、それ以外は何も打たずにPTYの上で1つのcommandを実行する。
///
/// 期待するpromptが現れる前に、timeout・読み取り不能・processの終了のいずれかに
/// 達した場合は、答えを送らずに`ExternalCommandNotConfirmed`として終える。答えを
/// 送った後は、`CommandOutcome`をそのまま返し、成否の判定は呼び出し側に委ねる。
pub(crate) fn run_pty_confirmed<O: Processes + Pty>(
    os: &O,
    clock: &dyn Clock,
    command: &PtyConfirmedCommand,
) -> Result<CommandOutcome> {
    let spec = command.as_capture_spec();
    let (mut controller, terminal) = open_pty(os).map_err(|error| spawn_failure(&spec, &error))?;

    let mut process = Command::new(&command.program);
    process.args(&command.args);
    apply_env(&mut process, command.env, None);
    attach(os, &mut process, &terminal).map_err(|error| spawn_failure(&spec, &error))?;

    let mut child = start_child(os, &mut process, &spec)?;

    // `process`と`terminal`は端末側を持ったまま、この関数の終わりまで残る。子が終わっても
    // 端末側は閉じないため、読み切りはWouldBlockで終わる（契約test C15a）。
    match drive(os, clock, &mut child, &mut controller, command) {
        Ok(outcome) => Ok(outcome),
        Err(error) => {
            // 報告より先に終わらせる。`drive`自身は子を終わらせない。
            terminate_child(os, &mut child);
            Err(error)
        }
    }
}

/// PTYを1つ開き、端末側をraw modeにする。順番は変えず、失敗はそこで諦める。
fn open_pty<O: Pty>(os: &O) -> std::io::Result<(O::Controller, O::Terminal)> {
    let controller = os.open_controller()?;
    os.close_on_exec(&controller)?;
    os.grant(&controller)?;
    os.unlock_terminal(&controller)?;
    let name = os.terminal_name(&controller)?;
    // 制御端末として奪わない。sbxm自身のsessionへ結び付けない。
    let terminal = os.open_terminal(&name)?;
    // 端末側をrawにする。echoと行編集は端末の機能であり、答えを送る打鍵ではない。
    let mut settings = os.settings(&terminal)?;
    os.make_raw(&mut settings);
    os.apply_settings(&terminal, &settings)?;
    os.set_size(&controller, ROWS, COLUMNS)?;
    // 読み取りは待たずに戻す。待ち時間の上限はdriveのloopが決める。
    os.controller_nonblocking(&controller)?;
    Ok((controller, terminal))
}

/// 子の3本のstreamを、端末側の複製へ向ける。
fn attach<O: Pty>(os: &O, process: &mut Command, terminal: &O::Terminal) -> std::io::Result<()> {
    process.stdin(os.terminal_stdio(terminal)?);
    process.stdout(os.terminal_stdio(terminal)?);
    process.stderr(os.terminal_stdio(terminal)?);
    Ok(())
}

/// promptを監視しながら子processの終わりまで読み、答えるべき瞬間にだけ書き込む。
/// 失敗を返すとき、子を終わらせるのは呼び出し側である。
fn drive<O: Processes + Pty>(
    os: &O,
    clock: &dyn Clock,
    child: &mut O::Child,
    controller: &mut O::Controller,
    command: &PtyConfirmedCommand,
) -> Result<CommandOutcome> {
    let prompt_deadline = clock.now().after(command.prompt_timeout);
    let overall_deadline = command
        .timeout
        .duration()
        .map(|limit| clock.now().after(limit));
    let mut buffer: Vec<u8> = Vec::new();
    let mut answered = false;
    let mut chunk = [0u8; 4096];

    loop {
        // 読めた分だけ足す。0も、まだ何も無いことも、読めなくなった端末も、新しく言ったことは
        // 無いとし、以後はprocessの終了を待つ。
        if let Ok(size) = controller.read(&mut chunk) {
            buffer.extend_from_slice(&chunk[..size]);
        }

        if !answered {
            if prompt_is_ready(&buffer, &command.expected_prompt) {
                if let Err(error) = controller
                    .write_all(command.answer.as_bytes())
                    .and_then(|()| controller.flush())
                {
                    return Err(not_confirmed(
                        command,
                        &format!("the answer could not be sent: {error}"),
                        &buffer,
                    ));
                }
                answered = true;
            } else if clock.now() >= prompt_deadline {
                return Err(not_confirmed(
                    command,
                    "the expected prompt did not appear in time",
                    &buffer,
                ));
            }
        }

        match os.check_exit(child) {
            Ok(Some(status)) => {
                drain_after_exit(controller, &mut buffer, command)?;
                return finish(command, status, buffer, answered);
            }
            Ok(None) => {}
            Err(error) => return Err(spawn_failure(&command.as_capture_spec(), &error)),
        }

        if overall_deadline.is_some_and(|deadline| clock.now() >= deadline) {
            return Err(timed_out(command));
        }

        clock.sleep(WAIT_POLL_INTERVAL);
    }
}

/// processが終わった後、PTYに残った出力を読み切る。
///
/// 親が端末側を持つ間、残りを読み終えたmasterのnonblocking readは`WouldBlock`を返す。誰も
/// 端末側を持たなければ、Linuxは`EIO`を、macOSは`0`を返す（契約test C15a/C15b）。どれも読み切りの
/// 終わりとし、それ以外の読み取り失敗は、runtimeの原文を失いかけたこととして外へ返す。
fn drain_after_exit<R: Read>(
    controller: &mut R,
    buffer: &mut Vec<u8>,
    command: &PtyConfirmedCommand,
) -> Result<()> {
    let mut chunk = [0u8; 4096];
    loop {
        match controller.read(&mut chunk) {
            Ok(0) => return Ok(()),
            Ok(size) => buffer.extend_from_slice(&chunk[..size]),
            Err(error)
                if error.kind() == std::io::ErrorKind::WouldBlock
                    || error.raw_os_error() == Some(Errno::IO.raw_os_error()) =>
            {
                return Ok(());
            }
            Err(error) => {
                return Err(unreadable(&command.as_capture_spec(), &error.to_string()));
            }
        }
    }
}

/// processが終わった時点の結果を決める。promptに答えられていなければ、exit codeが
/// 何であってもfail closedとする。
fn finish(
    command: &PtyConfirmedCommand,
    status: ExitStatus,
    buffer: Vec<u8>,
    answered: bool,
) -> Result<CommandOutcome> {
    if !answered {
        return Err(not_confirmed(
            command,
            "the process ended before the expected prompt appeared",
            &buffer,
        ));
    }
    let stderr_lossy = matches!(String::from_utf8_lossy(&buffer), std::borrow::Cow::Owned(_));
    Ok(CommandOutcome {
        program: command.program.clone(),
        args: command.args.clone(),
        working_dir: None,
        status,
        stdout: Vec::new(),
        stderr: buffer,
        stderr_lossy,
    })
}

/// これまでに読めたbyte列が、答えるべき確認promptそのものであるかを決める。
///
/// 部分一致では、期待文字列の直後に不正byteが続く出力や、期待文字列を前置きとして
/// 別の質問へ続く出力まで一致に見せかけてしまう。観測済みのbyte列が期待文字列と
/// 完全一致するか、入力待ち位置を示すASCIIの空白1個だけが末尾に付く場合だけ答える。
/// 2個以上の空白、tab、改行、unicode空白などは一致とみなさない。
fn prompt_is_ready(buffer: &[u8], expected_prompt: &str) -> bool {
    let expected = expected_prompt.as_bytes();
    buffer == expected
        || (buffer.len() == expected.len() + 1
            && buffer.starts_with(expected)
            && buffer[expected.len()] == b' ')
}

/// 有効なUTF-8として読める先頭部分。
///
/// promptの一致判定には使わない。診断へ載せる原文を組み立てるためだけに、途中で
/// 切れたmulti-byte文字や不正byteより手前までを取り出す。
fn valid_utf8_prefix(buffer: &[u8]) -> &str {
    match std::str::from_utf8(buffer) {
        Ok(text) => text,
        Err(error) => std::str::from_utf8(&buffer[..error.valid_up_to()]).unwrap_or(""),
    }
}

fn timed_out(command: &PtyConfirmedCommand) -> Error {
    let seconds = command
        .timeout
        .duration()
        .map_or(0, |limit| limit.as_secs());
    Error::single(
        Diagnostic::new(
            ErrorId::ExternalCommandTimeout,
            msg!(
                "error-external-command-timeout",
                program = command.program,
                seconds = seconds
            ),
        )
        .fact(Fact::command(&format!(
            "{} {}",
            command.program,
            command.args.join(" ")
        ))),
    )
}

/// 確認promptを安全に完了できなかった。exit codeの成否ではなく、答えられたかどうかで
/// 決める。
fn not_confirmed(command: &PtyConfirmedCommand, reason: &str, captured: &[u8]) -> Error {
    let seen = valid_utf8_prefix(captured);
    let detail = if seen.trim().is_empty() {
        reason.to_string()
    } else {
        format!("{reason}\n{seen}")
    };
    Error::single(
        Diagnostic::new(
            ErrorId::ExternalCommandNotConfirmed,
            msg!(
                "error-external-command-not-confirmed",
                subject = &command.subject
            ),
        )
        .fact(Fact::command(&format!(
            "{} {}",
            command.program,
            command.args.join(" ")
        )))
        .fact(Fact::cause(&detail))
        .remediation(msg!("remediation-external-command-not-confirmed")),
    )
}

#[cfg(test)]
#[path = "run_pty_confirmed_test.rs"]
mod run_pty_confirmed_test;
