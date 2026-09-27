//! 契約test: 子processの起動、終わらせ方、引き取り方、process groupについての仮定。
//!
//! 外部commandの実行は、起動や待ちの失敗をOSが返す値で見分け、子を終わらせる手順と置く
//! process groupをOSの約束に委ねる。ここで実OSに確かめる。scriptは`sh -c`で渡し、testが
//! 書いたfileをexecしない。書き込み中のfileをexecできないことを確かめるC9だけが例外である。
//!
//! 子を`wait`で引き取るのは、自分で終わる子と、SIGKILLで終わらせた子だけとする。入力の
//! 終わりで終わる子の終了も、pipeのEOFも、別のthreadのtestがforkした子が複製を閉じるまで
//! 来ないため、それらを待たない。

use std::fs::{File, Permissions};
use std::io::{self, BufRead, BufReader, ErrorKind, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::process::{Child, Command, Stdio};

use rustix::io::Errno;
use rustix::process::{Pid, Signal, WaitOptions, getpgid, waitpid};

use crate::testing::fs::temp_dir;
use crate::testing::outcome::{Checked, Refused, Required};

/// 入力pipeが開いている間は終わらない子。`read`はEOFまで戻らない。
fn reading_child() -> Checked<Child> {
    Ok(Command::new("sh")
        .args(["-c", "read _"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?)
}

/// C8: 起動する相手が無いときも、作業directoryが無いときも、起動は同じ`NotFound`で失敗する。
///
/// `spawn`は`NotFound`だけでは相手が居ないと言わない。作業directoryが無ければ、先にそれを
/// 名指しする。
#[test]
fn a_missing_program_and_a_missing_directory_both_fail_as_not_found() -> Checked {
    let dir = temp_dir()?;

    let program = Command::new("sbxm-contract-no-such-program")
        .stdin(Stdio::null())
        .spawn()
        .refused_because("the program does not exist")?;
    assert_eq!(program.kind(), ErrorKind::NotFound, "{program:?}");

    let directory = Command::new("true")
        .current_dir(dir.path().join("missing"))
        .stdin(Stdio::null())
        .spawn()
        .refused_because("the working directory does not exist")?;
    assert_eq!(directory.kind(), ErrorKind::NotFound, "{directory:?}");
    Ok(())
}

/// C8: directoryを起動する相手として渡すと、起動は`NotFound`以外で失敗する。
///
/// `spawn`は`NotFound`以外の失敗を、相手が居ないとは言わずにOSの原文のまま報告する。
#[test]
fn a_directory_given_as_the_program_fails_otherwise() -> Checked {
    let dir = temp_dir()?;

    let error = Command::new(dir.path())
        .stdin(Stdio::null())
        .spawn()
        .refused_because("a directory is not a program")?;
    assert_ne!(error.kind(), ErrorKind::NotFound, "{error:?}");
    Ok(())
}

/// C9: 書き込みのために開かれているfileは、Linuxではexecできず、`ETXTBSY`で拒まれる。
///
/// testが書いたscriptを直接execせず`sh <path>`で起動する決まり（`command_test.rs`の
/// `fake_script`）は、これによる。別のthreadのtestがforkした子は、自分のexecまで書き込み端の
/// 複製を持つため、書き終えて閉じた直後のfileもその間はexecできない。macOSのkernelがこれを
/// 拒むかはCIでしか確かめられないため、Linux以外では起動できた場合も受け入れる。拒まない
/// OSでも、この決まりは害を持たない。
#[test]
fn a_file_open_for_writing_cannot_be_executed() -> Checked {
    let dir = temp_dir()?;
    let path = dir.path().join("script");
    let mut writing = File::create(&path)?;
    writing.write_all(b"#!/bin/sh\nexit 0\n")?;
    writing.set_permissions(Permissions::from_mode(0o755))?;

    let started = Command::new(&path).stdin(Stdio::null()).spawn();
    drop(writing);
    refused_while_written(started)
}

/// 書き込み中のfileを起動した結果が、`ETXTBSY`による拒否であるか。
#[cfg(target_os = "linux")]
fn refused_while_written(started: io::Result<Child>) -> Checked {
    let error = started.refused_because("the file is open for writing")?;
    assert_eq!(
        error.raw_os_error(),
        Some(Errno::TXTBSY.raw_os_error()),
        "{error:?}"
    );
    Ok(())
}

/// 書き込み中のfileを起動した結果が、`ETXTBSY`による拒否か、scriptを最後まで走らせた起動か。
#[cfg(not(target_os = "linux"))]
fn refused_while_written(started: io::Result<Child>) -> Checked {
    match started {
        Err(error) => assert_eq!(
            error.raw_os_error(),
            Some(Errno::TXTBSY.raw_os_error()),
            "{error:?}"
        ),
        Ok(mut child) => {
            let status = child.wait()?;
            assert!(status.success(), "{status:?}");
        }
    }
    Ok(())
}

/// C10: 終わらせた子は直ちに終わり、待てば引き取られて残らない。終わらせるsignalはSIGKILL
/// であり、子は受け方を選べない。引き取った後の`waitpid`は`ECHILD`を返す。
///
/// `terminate_child`は猶予を与えずに終わらせ、終了statusを引き取ってzombieを残さない。
#[test]
fn an_ended_and_waited_child_is_gone() -> Checked {
    let mut child = reading_child()?;
    let _holding = child
        .stdin
        .take()
        .required_because("the input pipe was created")?;
    let pid = Pid::from_child(&child);

    child.kill()?;
    let status = child.wait()?;
    assert_eq!(status.signal(), Some(Signal::KILL.as_raw()), "{status:?}");

    let gone =
        waitpid(Some(pid), WaitOptions::NOHANG).refused_because("the child has been reaped")?;
    assert_eq!(gone, Errno::CHILD);
    Ok(())
}

/// C11: 別の場所で引き取られた子は、終わったかを尋ねることも待つこともできず、どちらも
/// `ECHILD`で失敗する。
///
/// `wait_with_limit`と`pump_until_exit`は、この失敗を受けると子を終わらせ、実行が成立しな
/// かったことをOSの原文とともに報告する。尋ねられない子を、終わったとも動いているともみなさない。
#[test]
fn a_child_reaped_elsewhere_cannot_be_checked_or_waited_for() -> Checked {
    let mut child = Command::new("true")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    let pid = Pid::from_child(&child);
    let reaped = waitpid(Some(pid), WaitOptions::empty())
        .required_because("the child is reaped outside its handle")?;
    assert_eq!(reaped.map(|(reaped, _)| reaped), Some(pid));

    let checked = child
        .try_wait()
        .refused_because("the child is no longer ours to check")?;
    assert_eq!(
        checked.raw_os_error(),
        Some(Errno::CHILD.raw_os_error()),
        "{checked:?}"
    );
    let waited = child
        .wait()
        .refused_because("the child is no longer ours to wait for")?;
    assert_eq!(
        waited.raw_os_error(),
        Some(Errno::CHILD.raw_os_error()),
        "{waited:?}"
    );
    Ok(())
}

/// C12: 専用のprocess groupへ置いた子は自分のpidをgroupの番号とし、その子孫も同じgroupに
/// 入る。対話しないshellはjob controlを持たず、背景の子孫を別のgroupへ移さない。
///
/// `run_inner`と`run_streaming`はCapture commandをこうして置く。端末からsbxmのforeground
/// groupへ届くCtrl-Cは、子にも、daemonになりうる子孫にも届かない。
#[test]
fn a_child_in_its_own_group_takes_its_descendants_along() -> Checked {
    // 子孫の`cat`は、このtestが持つ入力pipeを読み続ける。入力pipeを閉じるまで終わらない
    // ため、時間に頼らずに生かしておける。子孫のpidは、子が終わる前にstdoutへ書く。
    let mut child = Command::new("sh")
        .args(["-c", "exec 3<&0; cat <&3 >/dev/null & echo $!"])
        .process_group(0)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let holding = child
        .stdin
        .take()
        .required_because("the input pipe was created")?;
    let stdout = child
        .stdout
        .take()
        .required_because("the output pipe was created")?;
    let group = Pid::from_child(&child);
    let status = child.wait()?;
    assert!(status.success(), "{status:?}");

    // 子は終わっており、`echo`が書いた行はpipeにある。pipeのEOFは、子孫が出力の複製を
    // 閉じるまで来ないため、1行だけ読む。
    let mut said = String::new();
    BufReader::new(stdout).read_line(&mut said)?;
    let descendant = Pid::from_raw(
        said.trim()
            .parse()
            .required_because("the child names its descendant")?,
    )
    .required_because("a process id is positive")?;
    assert_eq!(
        getpgid(Some(descendant)).required_because("the descendant is alive")?,
        group
    );
    assert_ne!(
        getpgid(None).required_because("our own group")?,
        group,
        "the child left our group"
    );
    drop(holding);
    Ok(())
}

/// C13: process groupを選ばずに起動した子は、sbxmと同じgroupに残る。
///
/// `run_terminal_inner`は、端末へ出るcommandをこうして起動する。`run_relay`はこれに頼り、
/// 割り込みを見張らない。利用者のCtrl-Cは、sbxmへ届くのと同じく子へも届く。
#[test]
fn a_child_started_without_its_own_group_stays_in_ours() -> Checked {
    let mut child = reading_child()?;
    let _holding = child
        .stdin
        .take()
        .required_because("the input pipe was created")?;

    let group = getpgid(Some(Pid::from_child(&child))).required_because("the child is alive")?;
    child.kill()?;
    child.wait()?;
    assert_eq!(group, getpgid(None).required_because("our own group")?);
    Ok(())
}
