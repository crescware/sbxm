//! Capture commandの中断時に、直接の子だけが終わることを確認する。

use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::process::Command;
use std::time::{Duration, Instant};

#[test]
fn ctrl_c_does_not_reach_a_capture_descendant() -> Result<(), Box<dyn std::error::Error>> {
    let home = tempfile::tempdir()?;
    let bin = home.path().join("bin");
    std::fs::create_dir(&bin)?;
    let survivor = home.path().join("survivor");
    let sw_vers = bin.join("sw_vers");
    std::fs::write(
        &sw_vers,
        "#!/bin/sh\n\
         (sleep 1; printf alive > \"$SBXM_SURVIVOR\") &\n\
         kill -INT -\"$PPID\"\n\
         sleep 30\n",
    )?;
    std::fs::set_permissions(&sw_vers, std::fs::Permissions::from_mode(0o755))?;

    // sbxm自身をprocess group leaderにする。fake commandは`-$PPID`へsignalを送り、
    // 端末からforeground groupへ届くCtrl-Cと同じgroupを対象にする。
    let output = Command::new(env!("CARGO_BIN_EXE_sbxm"))
        .args(["--lang", "en", "status", "--global"])
        .env("HOME", home.path())
        .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
        .env("SBXM_SURVIVOR", &survivor)
        .env("NO_COLOR", "1")
        .process_group(0)
        .output()?;

    assert!(
        !output.status.success(),
        "the interrupted diagnostic should report the canceled command"
    );

    let deadline = Instant::now() + Duration::from_secs(5);
    while !survivor.exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        std::fs::read_to_string(&survivor)?,
        "alive",
        "Ctrl-C must not reach a capture descendant"
    );
    Ok(())
}
