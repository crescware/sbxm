use std::fs::File;
use std::io::{Read, Write};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use rustix::fs::{Mode, OFlags, fcntl_getfl, fcntl_setfl};
use rustix::io::{FdFlags, fcntl_setfd};
use rustix::pty::{OpenptFlags, grantpt, openpt, ptsname, unlockpt};
use rustix::termios::{OptionalActions, Winsize, tcgetattr, tcsetattr, tcsetwinsize};

use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;

use super::{
    CommandOutcome, EnvPolicy, PtyConfirmedCommand, spawn, spawn_failure, terminate_child,
};

/// promptを読む間隔。
const POLL_INTERVAL: Duration = Duration::from_millis(20);

/// 端末の大きさ。値を固定し、折り返しを実行環境から切り離す。
const ROWS: u16 = 24;
const COLUMNS: u16 = 120;

/// 確認promptにだけ答え、それ以外は何も打たずにPTYの上で1つのcommandを実行する。
///
/// 期待するpromptが現れる前に、timeout・読み取り不能・processの終了のいずれかに
/// 達した場合は、答えを送らずに`ExternalCommandNotConfirmed`として終える。答えを
/// 送った後は、`CommandOutcome`をそのまま返し、成否の判定は呼び出し側に委ねる。
pub(super) fn run_pty_confirmed(command: &PtyConfirmedCommand) -> Result<CommandOutcome> {
    let spec = command.as_capture_spec();
    let (controller, terminal) = open_pty().map_err(|error| spawn_failure(&spec, &error))?;

    let mut process = Command::new(&command.program);
    process.args(&command.args);
    if command.env == EnvPolicy::InheritWithoutSshAgent {
        process.env_remove("SSH_AUTH_SOCK");
    }
    let stdin = terminal
        .try_clone()
        .map_err(|error| spawn_failure(&spec, &error))?;
    let stdout = terminal
        .try_clone()
        .map_err(|error| spawn_failure(&spec, &error))?;
    process.stdin(Stdio::from(stdin));
    process.stdout(Stdio::from(stdout));
    process.stderr(Stdio::from(terminal));

    let mut child = spawn(&mut process, &spec)?;

    match drive(&mut child, controller, command) {
        Ok(outcome) => Ok(outcome),
        Err(error) => {
            terminate_child(&mut child);
            Err(error)
        }
    }
}

/// PTYを1つ開き、端末側をraw modeにする。
fn open_pty() -> std::io::Result<(File, File)> {
    let controller = openpt(OpenptFlags::RDWR | OpenptFlags::NOCTTY)?;
    fcntl_setfd(&controller, FdFlags::CLOEXEC)?;
    grantpt(&controller)?;
    unlockpt(&controller)?;
    let name = ptsname(&controller, Vec::new())?;
    // 制御端末として奪わない。sbxm自身のsessionへ結び付けない。
    let terminal = File::from(rustix::fs::open(
        &name,
        OFlags::RDWR | OFlags::NOCTTY | OFlags::CLOEXEC,
        Mode::empty(),
    )?);

    // 端末側をrawにする。echoと行編集は端末の機能であり、答えを送る打鍵ではない。
    let mut settings = tcgetattr(&terminal)?;
    settings.make_raw();
    tcsetattr(&terminal, OptionalActions::Now, &settings)?;
    tcsetwinsize(
        &controller,
        Winsize {
            ws_row: ROWS,
            ws_col: COLUMNS,
            ws_xpixel: 0,
            ws_ypixel: 0,
        },
    )?;

    let controller = File::from(controller);
    // 読み取りは待たずに戻す。待ち時間の上限はここのloopが決める。
    let flags = fcntl_getfl(&controller)?;
    fcntl_setfl(&controller, flags | OFlags::NONBLOCK)?;
    Ok((controller, terminal))
}

/// promptを監視しながら子processの終わりまで読み、答えるべき瞬間にだけ書き込む。
fn drive(
    child: &mut Child,
    mut controller: File,
    command: &PtyConfirmedCommand,
) -> Result<CommandOutcome> {
    let prompt_deadline = Instant::now() + command.prompt_timeout;
    let overall_deadline = command
        .timeout
        .duration()
        .map(|limit| Instant::now() + limit);
    let mut buffer: Vec<u8> = Vec::new();
    let mut answered = false;
    let mut chunk = [0u8; 4096];

    loop {
        match controller.read(&mut chunk) {
            Ok(0) => {}
            Ok(size) => buffer.extend_from_slice(&chunk[..size]),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            // 読めなくなった端末は、これ以上何も言わない相手として扱う。以後はprocessの
            // 終了だけを待つ。
            Err(_) => {}
        }

        if !answered {
            if valid_utf8_prefix(&buffer).contains(command.expected_prompt.as_str()) {
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
            } else if Instant::now() >= prompt_deadline {
                let _ = child.kill();
                let _ = child.wait();
                return Err(not_confirmed(
                    command,
                    "the expected prompt did not appear in time",
                    &buffer,
                ));
            }
        }

        match child.try_wait() {
            Ok(Some(status)) => {
                if let Ok(size) = controller.read(&mut chunk) {
                    buffer.extend_from_slice(&chunk[..size]);
                }
                return finish(command, status, buffer, answered);
            }
            Ok(None) => {}
            Err(error) => return Err(spawn_failure(&command.as_capture_spec(), &error)),
        }

        if overall_deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(timed_out(command));
        }

        std::thread::sleep(POLL_INTERVAL);
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

/// 有効なUTF-8として読める先頭部分。
///
/// 途中で切れたmulti-byte文字を無効と誤認しないよう、無効になった位置までを使う。
/// 破損したbyte列をpromptの一致に見せかけないため、lossy変換はしない。
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
