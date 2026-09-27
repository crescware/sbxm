//! 契約test C2: 待っている間にsignal handlerが走ると、`poll`は`Interrupted`で終わる。
//! signal-hookは`SA_RESTART`を付けてhandlerを置くが、`poll`は再開されない。
//!
//! Capture commandの実行中、`poll_pipes`は`Interrupted`を何も起きなかった待ちとして扱い、
//! loopを続ける。待ちの途中に届いたCtrl-Cは、待ちの期限を待たずに`pump_until_exit`の
//! 割り込みの確かめへ届く。
//!
//! libtestはtestをthreadで走らせる。unsafeを使わずに送れるsignalはprocess宛てだけであり、
//! threadのどれへ届くかはOSが選ぶ。`poll`で待つthreadへ届くとは限らない。そこでharnessを
//! 使わず、threadが`main`の1本しかないprocessで確かめる。
//!
//! 見張りは`SignalGuard`と同じくSIGINTへ置く。このbinaryには、ほかに見張りを置くtestが無い。
//! 待ちの期限は、`src/testing/wait_until.rs`の上限と同じ60秒とする。期限はhangを止めるために
//! あり、速さは確かめない。

mod outcome;

use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use rustix::event::{PollFd, PollFlags, Timespec};
use rustix::io::Errno;
use signal_hook::consts::SIGINT;

use outcome::{Checked, Required, Unmet};

/// 待ちの期限。
const WAIT_LIMIT: Timespec = Timespec {
    tv_sec: 60,
    tv_nsec: 0,
};

fn main() -> Checked {
    let arrived = Arc::new(AtomicBool::new(false));
    let registration = signal_hook::flag::register(SIGINT, Arc::clone(&arrived))
        .required_because("place the same watch as SignalGuard")?;
    // 誰も書かないpipe。書き込み端を持ち続けるため、閉じたことでも待ちは終わらない。
    let (idle, _silent) = std::io::pipe().required_because("open a pipe")?;

    // このprocessが居る間、SIGINTを送り続ける。どれかは`poll`で待っている間に届く。
    let mut sender = Command::new("sh")
        .args([
            "-c",
            r#"while kill -INT "$0" 2>/dev/null; do sleep 0.01; done"#,
        ])
        .arg(std::process::id().to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .required_because("start sending SIGINT")?;
    let mut watched = [PollFd::new(
        &idle,
        PollFlags::IN | PollFlags::HUP | PollFlags::ERR | PollFlags::NVAL,
    )];
    let waited = rustix::event::poll(&mut watched, Some(&WAIT_LIMIT));

    sender.kill().required_because("stop sending SIGINT")?;
    sender.wait().required_because("reap the sender")?;
    signal_hook::low_level::unregister(registration);

    if waited == Err(Errno::INTR) && arrived.load(Ordering::SeqCst) {
        return Ok(());
    }
    Err(Unmet::new(format!(
        "a handled SIGINT should end poll with EINTR, but poll gave {waited:?} \
         (the handler ran: {})",
        arrived.load(Ordering::SeqCst)
    )))
}
