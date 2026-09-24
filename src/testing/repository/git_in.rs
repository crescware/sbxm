use std::path::Path;
use std::process::Command;

use crate::testing::outcome::{Checked, Required, Unmet};

/// testが用意するrepositoryで、gitを走らせて標準出力を返す。
///
/// 利用者の設定に左右されないよう、global設定を読まず、名義を固定する。
pub fn git_in(directory: &Path, args: &[&str]) -> Checked<String> {
    let output = Command::new("git")
        .args([
            "-c",
            "user.name=Example User",
            "-c",
            "user.email=user@example.com",
            "-c",
            "init.defaultBranch=main",
            "-c",
            "advice.detachedHead=false",
        ])
        .args(args)
        .current_dir(directory)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .required_because("git runs")?;
    if !output.status.success() {
        return Err(Unmet::new(format!(
            "git {args:?} failed in {}: {}",
            directory.display(),
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
