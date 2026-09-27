//! 契約test: signalの受け方についての仮定。
//!
//! どのtestも、このprocessへsignalを送らない。送れば、同時に走る別のtestが置いたCtrl-Cの
//! 見張りを立てる。scriptの中のsignalは、そのscriptを走らせるshell自身へ送る。
//!
//! 次の仮定は、同じ理由でtestにしない。
//!
//! - C20: 見張りをすべて外しても、SIGINTはOSの既定の動作へ戻らない。signal-hookのhandlerが
//!   残り、何もせずに戻る（`signal-hook-registry` 1.4.8の`unregister`の注意書き）。最初の
//!   Capture commandが見張りを外した後、sbxm自身はSIGINTで終わらない。同じgroupの子には
//!   届く。確かめるには、このprocessへSIGINTを送るしかない。

use std::fs;
use std::os::unix::process::ExitStatusExt;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use signal_hook::consts::{SIGINT, SIGTERM};

use crate::testing::fs::temp_dir;
use crate::testing::outcome::{Checked, Required};

/// C19: Ctrl-Cの見張りは何度でも置け、置いた見張りはそれぞれ外せる。1度も送らない。
///
/// `SignalGuard`はCapture commandを実行するたびに見張りを置き、落とすときに外す。1つのprocess
/// の中で、置いて外すことを何度も繰り返す。
#[test]
fn an_interrupt_watch_can_be_placed_again_and_removed() -> Checked {
    let first = signal_hook::flag::register(SIGINT, Arc::new(AtomicBool::new(false)))?;
    let second = signal_hook::flag::register(SIGINT, Arc::new(AtomicBool::new(false)))?;

    assert!(signal_hook::low_level::unregister(second));
    assert!(signal_hook::low_level::unregister(first));
    Ok(())
}

/// C21: dashは、受け方を置いていないsignalで終わるとき、EXIT trapを走らせない。
///
/// `PLACE_FROM_STDIN`は、一時fileを消すtrapをEXITへ置くだけでなく、signalを`exit`へ変える
/// trapも置く。macOSの`/bin/sh`はbashであり、bashはこの場合もEXIT trapを走らせるため、dashを
/// 名指しする。
#[test]
fn dash_skips_the_exit_trap_when_a_signal_ends_it() -> Checked {
    let dir = temp_dir()?;
    let marker = dir.path().join("exit-trap-ran");

    let status = Command::new("dash")
        .args(["-c", r#"trap 'printf ran > "$0"' EXIT; kill -TERM $$"#])
        .arg(&marker)
        .stdin(Stdio::null())
        .status()?;
    assert_eq!(status.signal(), Some(SIGTERM), "{status:?}");
    assert!(!marker.exists(), "the EXIT trap ran");
    Ok(())
}

/// C21: signalを`exit`へ変えるtrapを置けば、dashはそのsignalを受けたときもEXIT trapを走らせる。
/// 終了statusは、trapの`exit`が決める。
///
/// `PLACE_FROM_STDIN`は`trap 'exit 143' HUP INT TERM`を置き、signalで止められても一時fileを
/// 消す。
#[test]
fn a_signal_turned_into_an_exit_runs_the_exit_trap_in_dash() -> Checked {
    let dir = temp_dir()?;
    let marker = dir.path().join("exit-trap-ran");

    let status = Command::new("dash")
        .args([
            "-c",
            r#"trap 'printf ran > "$0"' EXIT; trap 'exit 143' TERM; kill -TERM $$"#,
        ])
        .arg(&marker)
        .stdin(Stdio::null())
        .status()?;
    assert_eq!(status.code(), Some(143), "{status:?}");
    assert_eq!(
        fs::read(&marker).required_because("the EXIT trap ran")?,
        b"ran"
    );
    Ok(())
}

/// C22: trapを置いたdashは、`$(...)`の途中で届いたsignalを、置換を終えて代入してから処理する。
///
/// `PLACE_FROM_STDIN`を、signalの受け方を一時fileを作る`$(mktemp)`より前に置く順へ並べ替える
/// には、これが要る。置換の途中でsignalを受けても、trapは作った一時fileの名前を読める。
#[test]
fn a_trapped_signal_waits_for_the_command_substitution() -> Checked {
    let output = Command::new("dash")
        .args([
            "-c",
            r#"trap 'printf "%s" "$x"' TERM; x=$(kill -TERM $$; printf value)"#,
        ])
        .stdin(Stdio::null())
        .output()?;
    assert!(output.status.success(), "{:?}", output.status);
    assert_eq!(output.stdout, b"value");
    Ok(())
}
