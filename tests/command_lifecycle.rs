//! Capture commandの中断時に、専用process groupも終わることを確認する。

use std::os::unix::fs::PermissionsExt;
use std::process::Command;
use std::time::Duration;

#[test]
fn ctrl_c_does_not_leave_a_capture_command_running() -> Result<(), Box<dyn std::error::Error>> {
    let home = tempfile::tempdir()?;
    let bin = home.path().join("bin");
    std::fs::create_dir(&bin)?;
    let survivor = home.path().join("survivor");
    let sw_vers = bin.join("sw_vers");
    std::fs::write(
        &sw_vers,
        format!(
            "#!/bin/sh\nkill -INT \"$PPID\"\nsleep 2\nprintf alive > \"{}\"\n",
            survivor.display()
        ),
    )?;
    std::fs::set_permissions(&sw_vers, std::fs::Permissions::from_mode(0o755))?;

    let _output = Command::new(env!("CARGO_BIN_EXE_sbxm"))
        .args(["--lang", "en", "status", "--global"])
        .env("HOME", home.path())
        .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
        .output()?;

    std::thread::sleep(Duration::from_secs(3));
    assert!(
        !survivor.exists(),
        "Ctrl-C must terminate the capture command's process group"
    );
    Ok(())
}
