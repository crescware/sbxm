//! 契約test: 子processとつながるpipeについての仮定。
//!
//! Capture commandの実行は、pipeを待たずに読み書きし、返った値だけで次を決める。空、終わり、
//! 満杯、読み手の不在をOSがどう返すかを、`RealHost`の基本操作を通して実OSに確かめる。
//! 子processを要しない仮定は、`std::io::pipe`の端を子processとの端の型へ移して確かめる。
//!
//! 端を閉じても、相手にそれが届くのは、その端の複製がすべて閉じた後である。同じprocessの
//! 別のthreadのtestがforkした子は、自分のexecまで複製を持つ。閉じたことが届くのを見る仮定は、
//! 届くまで待つ。

use std::io::{self, ErrorKind, PipeReader, PipeWriter, Read, Write};
use std::os::fd::OwnedFd;
use std::process::{ChildStdin, ChildStdout, Command, Stdio};
use std::time::Duration;

use crate::boundary::host::{Pipes, Processes};
use crate::boundary::os::SystemClock;
use crate::testing::outcome::{Checked, Refused, Required, Unmet};
use crate::testing::wait_until::wait_until;

use super::RealHost;

/// どのOSの既定のpipeの容量よりも大きい長さ。Linuxの既定の容量はpage 16枚分であり、page
/// 1枚が65536 byteのmachineでも、この長さの16分の1に留まる。macOSは65536 byteまで伸びる。
const LARGER_THAN_ANY_PIPE: usize = 16 * 1024 * 1024;

/// 何も起きない待ちが終わる期限。
const SHORT_WAIT: Duration = Duration::from_millis(1);

/// 子のstdoutと同じ型の、待たずに読む読み取り端と、その書き込み端。
fn output_pipe() -> Checked<(ChildStdout, PipeWriter)> {
    let (reader, writer) = io::pipe()?;
    let stdout = ChildStdout::from(OwnedFd::from(reader));
    RealHost
        .stdout_nonblocking(&stdout)
        .required_because("read without waiting")?;
    Ok((stdout, writer))
}

/// 子のstdinと同じ型の、待たずに書く書き込み端と、その読み取り端。
fn input_pipe() -> Checked<(ChildStdin, PipeReader)> {
    let (reader, writer) = io::pipe()?;
    let stdin = ChildStdin::from(OwnedFd::from(writer));
    RealHost
        .stdin_nonblocking(&stdin)
        .required_because("write without waiting")?;
    Ok((stdin, reader))
}

/// C1: 待たずに読むpipeは、空なら`WouldBlock`を返し、届いたbyteがあればそれを返し、書き手が
/// すべて閉じれば`0`を返す。
///
/// `drain_pipe`は`WouldBlock`をまだ何も無いこととしてその回の読み取りを終え、`0`をEOFとして
/// pipeを閉じる。取り違えれば、動いている子の出力を捨てるか、閉じたpipeを読み続ける。
#[test]
fn a_nonblocking_read_tells_an_empty_pipe_from_a_closed_one() -> Checked {
    let (mut stdout, mut writer) = output_pipe()?;
    let mut buffer = [0_u8; 16];

    let empty = stdout
        .read(&mut buffer)
        .refused_because("an empty pipe has nothing to read")?;
    assert_eq!(empty.kind(), ErrorKind::WouldBlock, "{empty:?}");

    writer.write_all(b"out")?;
    let read = stdout.read(&mut buffer)?;
    assert_eq!(&buffer[..read], b"out");

    drop(writer);
    wait_until(&SystemClock, "the pipe to reach its end", || {
        match stdout.read(&mut buffer) {
            Ok(0) => Ok(Some(())),
            Err(error) if error.kind() == ErrorKind::WouldBlock => Ok(None),
            other => Err(Unmet::new(format!(
                "nothing more was written, but the pipe gave {other:?}"
            ))),
        }
    })
}

/// C3: 読み手が居なくなったpipeへの書き込みは`BrokenPipe`で失敗する。Rustのruntimeは
/// SIGPIPEを無視するため、書いたprocessは終わらない。
///
/// `InputFeed::feed`は`BrokenPipe`を、子が入力を読むのをやめたこととして書き込みを終え、
/// 結果を子の終了statusに委ねる。
#[test]
fn writing_to_a_pipe_nobody_reads_is_a_broken_pipe() -> Checked {
    let (mut stdin, reader) = input_pipe()?;
    drop(reader);

    // 読み取り端の複製が残る間は、書き込みはpipeへ入る。
    wait_until(&SystemClock, "the write to break", || {
        match stdin.write(b"x") {
            Err(error) if error.kind() == ErrorKind::BrokenPipe => Ok(Some(())),
            Ok(_) => Ok(None),
            Err(error) if error.kind() == ErrorKind::WouldBlock => Ok(None),
            Err(error) => Err(Unmet::new(format!("the write failed otherwise: {error:?}"))),
        }
    })
}

/// C4: 読まれないpipeへ待たずに書くと、入る分だけを受け取ってその長さを返し、満杯になれば
/// `WouldBlock`を返す。容量の値は仮定せず、どのOSの既定の容量をも超える長さを1度に書く。
///
/// `InputFeed::feed`は書けた長さだけ進め、`WouldBlock`でその回の書き込みを終え、子が読むのを
/// 待つ。
#[test]
fn a_full_pipe_takes_part_of_a_write_and_then_would_block() -> Checked {
    let (mut stdin, _unread) = input_pipe()?;
    let bytes = vec![b'x'; LARGER_THAN_ANY_PIPE];

    let taken = stdin.write(&bytes)?;
    assert!(
        taken > 0 && taken < bytes.len(),
        "{taken} of {} bytes",
        bytes.len()
    );
    let full = stdin
        .write(&bytes[taken..])
        .refused_because("the pipe is full")?;
    assert_eq!(full.kind(), ErrorKind::WouldBlock, "{full:?}");
    Ok(())
}

/// C5: 書き手がすべて閉じたpipeは、読み取り端を見張る待ちを、読めるか閉じたかとして起こす。
///
/// `poll_pipes`は、子が出力を閉じたときも待ちを切り上げ、`pump_until_exit`は続けて読んで
/// EOFを知る。どの端が起こしたかは見ない。
#[test]
fn a_pipe_whose_writers_all_closed_shows_up_in_the_wait() -> Checked {
    let (stdout, writer) = output_pipe()?;
    drop(writer);

    wait_until(&SystemClock, "the closed pipe to show up", || {
        let mut watched = [RealHost.watch_stdout(&stdout)];
        let ready = RealHost
            .wait_for_io(&mut watched, Duration::ZERO)
            .required_because("look at the pipe")?;
        Ok((ready == 1).then_some(()))
    })
}

/// C5: 何も起きない待ちは、期限が来れば何も起きなかったとして`0`を返す。見張る端が1つも
/// 無くても失敗しない。
///
/// `poll_pipes`は、動いている子が何も書かない間も、両方の出力を閉じた後も、loopの1回ごとに
/// 同じ待ちを置く。失敗すれば、黙っている子の実行を読み取りの失敗として終わらせる。
#[test]
fn a_wait_in_which_nothing_happens_ends_with_nothing_at_its_limit() -> Checked {
    let (stdout, _silent) = output_pipe()?;

    let mut idle = [RealHost.watch_stdout(&stdout)];
    let ready = RealHost
        .wait_for_io(&mut idle, SHORT_WAIT)
        .required_because("wait on a pipe nobody writes")?;
    assert_eq!(ready, 0);

    let ready = RealHost
        .wait_for_io(&mut [], SHORT_WAIT)
        .required_because("wait on nothing")?;
    assert_eq!(ready, 0);
    Ok(())
}

/// C7: 直接の子が終わっても、pipeの書き込み端を引き継いだ子孫が生きている間、読み取り端は
/// EOFにならず、待たずに読めば`WouldBlock`を返す。子孫が手放せばEOFになる。
///
/// `pump_until_exit`は、EOFではなく直接の子の終了で読み取りを止め、読み取り端を閉じる。EOFを
/// 待てば、daemonのように残る子孫が実行を終わらせない。
#[test]
fn a_descendant_keeps_a_pipe_open_after_the_direct_child_exits() -> Checked {
    // 子孫の`cat`は、このtestが持つ入力pipeを読み続け、出力pipeを引き継いだまま残る。入力
    // pipeを閉じるまで終わらないため、時間に頼らずに生かしておける。
    let mut command = Command::new("sh");
    command
        .args(["-c", "exec 3<&0; cat <&3 &"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = RealHost.start(&mut command)?;
    // 子孫が先に終わらないよう、入力pipeは子を待つ前に引き取る。
    let holding = RealHost
        .take_stdin(&mut child)
        .required_because("the input pipe was created")?;
    let mut stdout = RealHost
        .take_stdout(&mut child)
        .required_because("the output pipe was created")?;
    RealHost
        .stdout_nonblocking(&stdout)
        .required_because("read without waiting")?;
    let status = RealHost.wait_exit(&mut child)?;
    assert!(status.success(), "{status:?}");

    let mut buffer = [0_u8; 16];
    let held = stdout
        .read(&mut buffer)
        .refused_because("the descendant still holds the pipe")?;
    assert_eq!(held.kind(), ErrorKind::WouldBlock, "{held:?}");

    drop(holding);
    wait_until(&SystemClock, "the pipe to reach its end", || {
        match stdout.read(&mut buffer) {
            Ok(0) => Ok(Some(())),
            Err(error) if error.kind() == ErrorKind::WouldBlock => Ok(None),
            other => Err(Unmet::new(format!(
                "the descendant wrote nothing, but the pipe gave {other:?}"
            ))),
        }
    })
}
