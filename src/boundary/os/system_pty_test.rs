//! 契約test: PTYについての仮定。
//!
//! 確認promptへの応答は、PTYの親の側を待たずに読み、読めた値だけで次を決める。PTYは
//! `RealHost`の`Pty`実装を通し、`run_pty_confirmed`の`open_pty`と同じ手順で開き、子の3本の
//! streamを端末側へ向ける。親の側は`poll`で待たず、間を置いて読み直す。macOSの`poll`は
//! 端末のdeviceを扱わない。

use std::fs::File;
use std::io::{self, ErrorKind, Read, Write};
use std::process::{Child, Command};

use rustix::io::{Errno, FdFlags, fcntl_getfd};
use rustix::termios::tcgetwinsize;

use crate::boundary::host::Pty;
use crate::boundary::os::SystemClock;
use crate::testing::outcome::{Checked, Refused, Required, Unmet};
use crate::testing::wait_until::wait_until;

use super::RealHost;

/// `run_pty_confirmed`が固定する端末の大きさ。
const ROWS: u16 = 24;
const COLUMNS: u16 = 120;

/// `run_pty_confirmed`の`open_pty`と同じ手順で開いたPTYの、親が読み書きする側と端末側。
fn open_pty() -> Checked<(File, File)> {
    let controller = RealHost
        .open_controller()
        .required_because("open the controller")?;
    RealHost
        .close_on_exec(&controller)
        .required_because("close it on exec")?;
    RealHost
        .grant(&controller)
        .required_because("grant the terminal")?;
    RealHost
        .unlock_terminal(&controller)
        .required_because("unlock the terminal")?;
    let name = RealHost
        .terminal_name(&controller)
        .required_because("name the terminal")?;
    let terminal = RealHost
        .open_terminal(&name)
        .required_because("open the terminal")?;
    let mut settings = RealHost
        .settings(&terminal)
        .required_because("read the settings")?;
    RealHost.make_raw(&mut settings);
    RealHost
        .apply_settings(&terminal, &settings)
        .required_because("make it raw")?;
    RealHost
        .set_size(&controller, ROWS, COLUMNS)
        .required_because("set the size")?;
    RealHost
        .controller_nonblocking(&controller)
        .required_because("stop blocking")?;
    Ok((controller, terminal))
}

/// `run_pty_confirmed`と同じく、子の3本のstreamを端末側へ向けて`script`を起動する。
///
/// 子へ渡す複製は、起動の後に閉じる。親が端末側を持ち続けるかは、`terminal`を持つ呼び出し側が
/// 決める。
fn start_on(terminal: &File, script: &str) -> Checked<Child> {
    Ok(Command::new("sh")
        .args(["-c", script])
        .stdin(RealHost.terminal_stdio(terminal)?)
        .stdout(RealHost.terminal_stdio(terminal)?)
        .stderr(RealHost.terminal_stdio(terminal)?)
        .spawn()?)
}

/// 読めたbyteを`output`へ足し、`WouldBlock`をまだ何も無いこととする1回の読み取り。
///
/// それ以外の結果は、読み取りの終わりとして返す。
fn read_once(controller: &mut File, output: &mut Vec<u8>) -> Option<io::Result<usize>> {
    let mut chunk = [0_u8; 64];
    match controller.read(&mut chunk) {
        Ok(0) => Some(Ok(0)),
        Ok(read) => {
            output.extend_from_slice(&chunk[..read]);
            None
        }
        Err(error) if error.kind() == ErrorKind::WouldBlock => None,
        Err(error) => Some(Err(error)),
    }
}

/// 読み取りの失敗が`EIO`か。
fn is_eio(read: &io::Result<usize>) -> bool {
    matches!(read, Err(error) if error.raw_os_error() == Some(Errno::IO.raw_os_error()))
}

/// 端末側をすべて閉じた後の読み取りが、終わりを告げるものか。Linuxは`EIO`を返す。
#[cfg(target_os = "linux")]
fn reports_the_end(read: &io::Result<usize>) -> bool {
    is_eio(read)
}

/// 端末側をすべて閉じた後の読み取りが、終わりを告げるものか。macOSは`0`を返すと見込むが、
/// CIでしか確かめられないため、`EIO`も受け入れる。
#[cfg(not(target_os = "linux"))]
fn reports_the_end(read: &io::Result<usize>) -> bool {
    matches!(read, Ok(0)) || is_eio(read)
}

/// C14: 開いたPTYの両端は、execで閉じる。`openpt`がCLOEXECを付けるとは限らないため、親の側は
/// 開いた後に付け、端末側は開くときに付ける。
///
/// `run_pty_confirmed`はこの手順で開く。PTYの上で起動する子も、ほかに起動する子processも、
/// 渡したstreamのほかは、このPTYの端を引き継がない。
#[test]
fn both_ends_of_an_opened_pty_close_on_exec() -> Checked {
    let (controller, terminal) = open_pty()?;

    let controller = fcntl_getfd(&controller).required_because("read the controller")?;
    assert!(controller.contains(FdFlags::CLOEXEC), "{controller:?}");
    let terminal = fcntl_getfd(&terminal).required_because("read the terminal")?;
    assert!(terminal.contains(FdFlags::CLOEXEC), "{terminal:?}");
    Ok(())
}

/// T5: 待たずに読む親の側は、何も届いていなければ`WouldBlock`を返す。
///
/// `drive`は、promptがまだ無いことを`WouldBlock`で知り、間を置いて読み直す。
#[test]
fn an_empty_nonblocking_controller_would_block() -> Checked {
    let (mut controller, _terminal) = open_pty()?;

    let mut chunk = [0_u8; 64];
    let error = controller
        .read(&mut chunk)
        .refused_because("nothing has been written to the terminal")?;
    assert_eq!(error.kind(), ErrorKind::WouldBlock, "{error:?}");
    Ok(())
}

/// C15a: 親が端末側を持つ間は、子が終わっても、親の側は子の出力を返した後に`WouldBlock`を
/// 返し、終わりを告げない。
///
/// `run_pty_confirmed`は端末側を最後まで持つ。子の終了を知った後の`drain_after_exit`は、
/// `WouldBlock`を読み切りの終わりとする。
#[test]
fn a_controller_whose_terminal_is_still_held_would_block_after_the_output() -> Checked {
    let (mut controller, terminal) = open_pty()?;
    let mut child = start_on(&terminal, "printf done")?;
    let status = child.wait()?;
    assert!(status.success(), "{status:?}");

    let mut output = Vec::new();
    wait_until(&SystemClock, "the output of the child", || match read_once(
        &mut controller,
        &mut output,
    ) {
        Some(end) => Err(Unmet::new(format!(
            "the terminal is still held, but the controller gave {end:?}"
        ))),
        None => Ok((output.len() >= b"done".len()).then_some(())),
    })?;
    assert_eq!(output, b"done");

    let mut chunk = [0_u8; 64];
    let after = controller
        .read(&mut chunk)
        .refused_because("nothing follows the output")?;
    assert_eq!(after.kind(), ErrorKind::WouldBlock, "{after:?}");
    drop(terminal);
    Ok(())
}

/// C15b: 端末側を誰も持たなくなると、親の側は残りの出力を返した後に終わりを告げる。
/// `WouldBlock`のままにはならない。終わりは、Linuxでは`EIO`である。macOSは`0`を返すと
/// 見込むが、CIでしか確かめられないため、Linux以外ではどちらも終わりとして受け入れる。
///
/// `drain_after_exit`は、`EIO`も`0`も読み切りの終わりとする。今の`run_pty_confirmed`は端末側を
/// 持ち続けるため、子が終わった後にこの終わりを読むことはない（C15a）。
#[test]
fn a_controller_whose_terminals_all_closed_reports_the_end() -> Checked {
    let (mut controller, terminal) = open_pty()?;
    let mut child = start_on(&terminal, "printf done")?;
    drop(terminal);

    // 端末側を最後に閉じる子は、残りが読まれるまで閉じ終わらないことがある。終わりまで
    // 読んでから、子を引き取る。
    let mut output = Vec::new();
    let end = wait_until(&SystemClock, "the end of the terminal", || {
        Ok(read_once(&mut controller, &mut output))
    })?;
    let status = child.wait()?;
    assert!(status.success(), "{status:?}");
    assert_eq!(output, b"done");
    assert!(reports_the_end(&end), "{end:?}");
    Ok(())
}

/// C16: rawにした端末では、親の側へ書いたbyteがそのまま子に届く。echoも改行の変換も無い。
///
/// `drive`は、期待したpromptが現れたときだけ答えのbyte列を1度書き、runtimeはそれを打鍵どおりに
/// 読む。echoがあれば、書いた答えがその後の出力に混じり、runtimeの原文として残る。
#[test]
fn bytes_written_to_the_controller_reach_a_raw_terminal_unchanged() -> Checked {
    let (mut controller, terminal) = open_pty()?;
    let mut child = start_on(&terminal, r#"read reply; printf "<%s>" "$reply""#)?;
    controller.write_all(b"y\n")?;

    // echoがあれば、それは子の返事より先に届く。返事の終わりまで読めば全部が揃う。
    let mut output = Vec::new();
    wait_until(&SystemClock, "the reply of the child", || {
        match read_once(&mut controller, &mut output) {
            Some(end) => Err(Unmet::new(format!(
                "the controller gave {end:?} before the reply"
            ))),
            None => Ok(output.ends_with(b">").then_some(())),
        }
    })?;
    let status = child.wait()?;
    assert!(status.success(), "{status:?}");
    assert_eq!(output, b"<y>", "{:?}", String::from_utf8_lossy(&output));
    drop(terminal);
    Ok(())
}

/// C17: 親の側へ設定した大きさを、端末側がそのまま報告する。
///
/// `run_pty_confirmed`は大きさを固定し、runtimeが出力を折り返す幅を実行環境から切り離す。
#[test]
fn the_size_set_on_the_controller_is_what_the_terminal_reports() -> Checked {
    let (_controller, terminal) = open_pty()?;

    let size = tcgetwinsize(&terminal).required_because("read the size")?;
    assert_eq!((size.ws_row, size.ws_col), (ROWS, COLUMNS));
    Ok(())
}
