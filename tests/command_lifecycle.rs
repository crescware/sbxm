//! Capture commandを中断したsbxmが、直接の子だけを終わらせることを確認する。
//!
//! sbxmはcapture commandの実行中に受けたSIGINTを中断として扱い、直接の子を終わらせて
//! 失敗を報告する。子のprocess groupへはsignalを送らず、子孫までは終わらせない。ここでは、
//! 直接の子が消えるのを待ってから子孫が印を残し、印が現れることでそれを確かめる。sbxmが
//! groupごと終わらせれば、子孫も同時に終わり、印は現れない。
//!
//! Ctrl-Cが子孫へ届かないことは、このtestでは確かめきれない。非対話のshellは背景で走らせる
//! jobのSIGINTを無視させる。子孫がsbxmと同じprocess groupにいても、無視を置いた後に届いた
//! Ctrl-Cでは終わらない。無視を置く前に届けば終わるため落ちることはあるが、それは時機に
//! よる。子孫を端末のCtrl-Cから切り離すのは、capture commandの子を専用のprocess groupへ
//! 置くことであり、それはprocess groupそのものを見なければ確かめられない。

mod fake_tool;
mod outcome;
mod wait_until;

use fake_tool::install_fake_tool;
use outcome::{Checked, Required};
use wait_until::wait_until;

use std::os::unix::process::CommandExt;
use std::process::Command;

#[test]
fn an_interrupted_capture_command_ends_only_its_direct_child() -> Checked {
    let home = tempfile::tempdir().required_because("temporary home")?;
    let bin = home.path().join("bin");
    std::fs::create_dir(&bin).required_because("the fake bin directory is created")?;
    let survivor = home.path().join("survivor");
    install_fake_tool(
        &bin,
        "sw_vers",
        // 子孫は、直接の子であるこのshellが消えるまで待ってから印を残す。印の有無を決めるのは、
        // sbxmが直接の子を終わらせたときに子孫が生きているかどうかだけであり、時機ではない。
        // subshellの`$$`は元のshellを指す。
        // 印は書き終えてから名前を付ける。redirectは中身より先に空のfileを作るため、書いて
        // いる途中の印を読むと、子孫が生きていても空に見える。
        // 直接の子は`exec`で`sleep`になり、sbxmに終わらされるまで待つ。`sleep`を子として
        // 起こすと、直接の子が終わらされた後も残り続ける。
        "parent=$$\n\
         (while kill -0 \"$parent\" 2>/dev/null; do sleep 0.01; done; printf alive > \"$SBXM_SURVIVOR.part\"; mv \"$SBXM_SURVIVOR.part\" \"$SBXM_SURVIVOR\") &\n\
         kill -INT -\"$PPID\"\n\
         exec sleep 30\n",
    )?;

    // sbxm自身をprocess group leaderにする。fake commandは`-$PPID`へsignalを送り、
    // 端末からforeground groupへ届くCtrl-Cと同じgroupを対象にする。
    let output = Command::new(env!("CARGO_BIN_EXE_sbxm"))
        .args(["--lang", "en", "status", "--global"])
        .env("HOME", home.path())
        .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
        .env("SBXM_SURVIVOR", &survivor)
        .env("NO_COLOR", "1")
        .process_group(0)
        .output()
        .required_because("sbxm runs")?;

    assert!(
        !output.status.success(),
        "the interrupted diagnostic should report the canceled command"
    );

    wait_until("the capture descendant to leave its marker", || {
        Ok(survivor.exists().then_some(()))
    })?;
    assert_eq!(
        std::fs::read_to_string(&survivor).required_because("the descendant left its marker")?,
        "alive",
        "an interrupted capture command must not take its descendants along"
    );
    Ok(())
}
