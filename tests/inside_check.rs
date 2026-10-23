//! Sandboxの中の確認をまとめる手順が、commandごとの結果を契約どおりに並べることの確認。
//!
//! sbxmはこの手順を`sbx exec`でSandboxのshellへ渡す。ここでは同じ本文をこのhostのshellで
//! 走らせ、並べ方が`parse_inside_check`の読み方と同じであることを確かめる。期待する
//! byte列は、`src/boundary/host/protocol/inside/inside_test.rs`が読み方の側から固定する
//! ものと同じである。
//!
//! 本文は`src/boundary/host/protocol/inside/inside_check.sh`から読む。本番の定数も同じ
//! fileを読むため、ここで走らせる手順は、sbxmがSandboxへ渡す手順と同じbyte列である。

mod outcome;

use outcome::{Checked, Required};

use std::process::{Command, Output, Stdio};

/// Sandboxの中で、受け取ったcommandを順に走らせて結果を並べる手順。本番の
/// `INSIDE_CHECK`と同じfileを読む。
const INSIDE_CHECK: &str = include_str!("../src/boundary/host/protocol/inside/inside_check.sh");

/// 手順を、このhostのshellで`args`を渡して走らせる。
fn run(args: &[&str]) -> Checked<Output> {
    run_with(args, |command| command)
}

fn run_with(
    args: &[&str],
    configure: impl FnOnce(&mut Command) -> &mut Command,
) -> Checked<Output> {
    let mut command = Command::new("sh");
    command.arg("-c").arg(INSIDE_CHECK).arg("sh").args(args);
    configure(&mut command)
        .output()
        .required_because("the shell runs the check")
}

#[test]
fn each_command_is_framed_with_its_status_stdout_and_stderr_as_written() -> Checked {
    // 2つ目は末尾の改行を持たず、3つ目はNULを含む。どちらもcommandが書いたとおりに届く。
    let output = run(&[
        "printf",
        "hello\\n",
        "",
        "sh",
        "-c",
        "printf abc; printf 'not found\\n' >&2; exit 2",
        "",
        "printf",
        "a\\000b\\000",
        "",
    ])?;
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        output.stdout,
        b"0 0 6 0\nhello\n1 2 3 10\nabcnot found\n2 0 4 0\na\0b\0"
    );
    assert!(output.stderr.is_empty(), "{:?}", output.stderr);
    Ok(())
}

#[test]
fn a_command_that_cannot_be_found_is_recorded_rather_than_ending_the_check() -> Checked {
    let output = run(&["sbxm-no-such-command", "", "printf", "after\\n", ""])?;
    assert_eq!(output.status.code(), Some(0));
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.starts_with("0 127 0 "), "{text:?}");
    assert!(text.ends_with("1 0 6 0\nafter\n"), "{text:?}");
    Ok(())
}

#[test]
fn arguments_a_command_is_quoted_from_are_passed_as_one_word_each() -> Checked {
    // 空白や引用符を含む引数も、1語のまま届く。
    let output = run(&["printf", "%s|", "two words", "it's", "", ""])?;
    assert_eq!(output.stdout, b"0 0 15 0\ntwo words|it's|");
    Ok(())
}

#[test]
fn no_command_produces_no_output() -> Checked {
    let output = run(&[])?;
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stdout.is_empty());
    Ok(())
}

#[test]
fn arguments_that_are_not_separated_end_the_check_without_a_record() -> Checked {
    // 区切りの無い末尾も、空のcommandも、手順の呼び方の誤りであり、何も並べない。
    for args in [&["printf", "x"][..], &["", "printf", "x", ""][..]] {
        let output = run(args)?;
        assert_eq!(output.status.code(), Some(64), "{args:?}");
        assert!(output.stdout.is_empty(), "{args:?}: {:?}", output.stdout);
    }
    Ok(())
}

#[test]
fn the_commands_do_not_read_the_stdin_the_check_was_given() -> Checked {
    let temp = tempfile::tempdir().required_because("a temporary directory for stdin")?;
    let path = temp.path().join("stdin");
    std::fs::write(&path, b"never read\n").required_because("stdin is prepared")?;
    let stdin = std::fs::File::open(&path).required_because("stdin opens")?;
    let output = run_with(&["cat", ""], |command| command.stdin(Stdio::from(stdin)))?;
    assert_eq!(output.stdout, b"0 0 0 0\n");
    Ok(())
}

#[test]
fn the_temporary_directory_is_removed_when_the_check_ends() -> Checked {
    let temp = tempfile::tempdir().required_because("a temporary directory for TMPDIR")?;
    let output = run_with(&["printf", "x", ""], |command| {
        command.env("TMPDIR", temp.path())
    })?;
    assert_eq!(output.stdout, b"0 0 1 0\nx");
    let left: Vec<_> = std::fs::read_dir(temp.path())
        .required_because("TMPDIR is readable")?
        .collect();
    assert!(left.is_empty(), "{left:?}");
    Ok(())
}
