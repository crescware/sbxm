use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

use crate::testing::outcome::{Checked, Required};

use super::WRITE;

const FAILURES: &str = r#"
fail() { command printf '%s\n' 'No space left on device' >&2; return 1; }
mktemp() { [ "$FAIL_AT" != create ] || { fail; return 1; }; command mktemp "$@"; }
printf() {
  if [ "$FAIL_AT" = write ]; then command printf '%s' partial; fail; return 1; fi
  command printf "$@"
}
sync() { [ "$FAIL_AT" != sync ] || { fail; return 1; }; }
mv() { [ "$FAIL_AT" != rename ] || { fail; return 1; }; command mv "$@"; }
"#;

#[test]
fn failed_creation_partial_write_sync_and_rename_preserve_the_old_configuration() -> Checked {
    for existing in [false, true] {
        for stage in ["create", "write", "sync", "rename"] {
            let directory = tempfile::tempdir().required()?;
            let target = directory.path().join("token.sh");
            if existing {
                fs::write(&target, b"old complete configuration\n").required()?;
            }
            let output = Command::new("sh")
                .args([
                    "-c",
                    &format!("{FAILURES}\n{WRITE}"),
                    "sh",
                    "marker",
                    "first",
                    "second",
                ])
                .arg(&target)
                .env("FAIL_AT", stage)
                .output()
                .required()?;
            assert!(!output.status.success(), "{existing} {stage}");
            assert!(String::from_utf8_lossy(&output.stderr).contains("No space left on device"));
            if existing {
                assert_eq!(
                    fs::read(&target).required()?,
                    b"old complete configuration\n"
                );
            } else {
                assert!(!target.exists());
            }
            assert_eq!(
                fs::read_dir(directory.path()).required()?.count(),
                usize::from(existing)
            );
        }
    }
    Ok(())
}

#[test]
fn a_complete_replacement_keeps_the_three_literal_lines_readable_by_login_shells() -> Checked {
    let directory = tempfile::tempdir().required()?;
    let target = directory.path().join("token.sh");
    fs::write(&target, b"old").required()?;
    let output = Command::new("sh")
        .args([
            "-c",
            &format!("{FAILURES}\n{WRITE}"),
            "sh",
            "marker",
            "literal $(false)",
            "last",
        ])
        .arg(&target)
        .env("FAIL_AT", "none")
        .output()
        .required()?;
    assert!(output.status.success(), "{:?}", output.stderr);
    assert_eq!(
        fs::read(&target).required()?,
        b"marker\nliteral $(false)\nlast\n"
    );
    assert_eq!(
        fs::metadata(&target).required()?.permissions().mode() & 0o777,
        0o644
    );
    assert_eq!(fs::read_dir(directory.path()).required()?.count(), 1);
    Ok(())
}
