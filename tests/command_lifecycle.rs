//! Capture commandの中断時に、直接の子だけが終わることを確認する。

mod fake_tool;
mod outcome;

use fake_tool::install_fake_tool;
use outcome::{Checked, Required};

use std::os::unix::process::CommandExt;
use std::process::Command;
use std::time::{Duration, Instant};

#[test]
fn ctrl_c_does_not_reach_a_capture_descendant() -> Checked {
    let home = tempfile::tempdir().required_because("temporary home")?;
    let bin = home.path().join("bin");
    std::fs::create_dir(&bin).required_because("the fake bin directory is created")?;
    let survivor = home.path().join("survivor");
    install_fake_tool(
        &bin,
        "sw_vers",
        // 印は書き終えてから名前を付ける。redirectは中身より先に空のfileを作るため、
        // 書いている途中の印を読むと、子孫が生きていても空に見える。
        "(sleep 1; printf alive > \"$SBXM_SURVIVOR.part\"; mv \"$SBXM_SURVIVOR.part\" \"$SBXM_SURVIVOR\") &\n\
         kill -INT -\"$PPID\"\n\
         sleep 30\n",
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

    let deadline = Instant::now() + Duration::from_secs(5);
    while !survivor.exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        std::fs::read_to_string(&survivor).required_because("the descendant left its marker")?,
        "alive",
        "Ctrl-C must not reach a capture descendant"
    );
    Ok(())
}
