//! `cargo xtask markdown`の公開契約。
//!
//! 一時directoryにgit repositoryを作り、xtaskの実行fileを`output()`で終わりまで待つ。
//! 終了codeは、規約を満たせば`0`、違反があれば`1`、引数が読めなければ`2`とする。

// 前提とした値の取り出しは、repositoryの統合testと同じ実体を取り込む。
#[path = "../../tests/outcome/mod.rs"]
mod outcome;

use std::path::Path;
use std::process::{Command, Output};

use outcome::{Checked, Required};

struct Run {
    stdout: String,
    stderr: String,
    code: i32,
}

impl Run {
    fn from(output: &Output) -> Checked<Run> {
        Ok(Run {
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            code: output
                .status
                .code()
                .required_because("the process exits normally")?,
        })
    }
}

/// `cwd`でxtaskを実行する。gitの設定は利用者のものを読まない。
fn xtask(cwd: &Path, arguments: &[&str]) -> Checked<Run> {
    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args(arguments)
        .current_dir(cwd)
        .env("HOME", cwd)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .required_because("xtask runs to completion")?;
    Run::from(&output)
}

fn git(cwd: &Path, arguments: &[&str]) -> Checked {
    let status = Command::new("git")
        .args(arguments)
        .current_dir(cwd)
        .env("HOME", cwd)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .required_because("git runs to completion")?
        .status;
    status
        .success()
        .then_some(())
        .required_because("git succeeds")
}

fn write(dir: &Path, name: &str, text: impl AsRef<[u8]>) -> Checked {
    std::fs::write(dir.join(name), text).required_because("the fixture file is written")
}

fn read(dir: &Path, name: &str) -> Checked<String> {
    std::fs::read_to_string(dir.join(name)).required_because("the file is readable")
}

/// 追跡中、未追跡、ignoreされたfileを1つずつ持つrepository。追跡中と未追跡は文の途中で
/// 改行している。
fn repository_with_wrapped_files() -> Checked<tempfile::TempDir> {
    let dir = tempfile::tempdir().required_because("temporary directory")?;
    git(dir.path(), &["init", "-q"])?;
    write(dir.path(), "tracked.md", "# t\n\nfirst line\nsecond line\n")?;
    git(dir.path(), &["add", "tracked.md"])?;
    write(dir.path(), "untracked.md", "日本語の\n文章。\n")?;
    write(dir.path(), ".gitignore", "ignored.md\n")?;
    write(dir.path(), "ignored.md", "a\nb\n")?;
    Ok(dir)
}

#[test]
fn wrapped_lines_in_tracked_and_untracked_files_are_reported_and_ignored_files_are_skipped()
-> Checked {
    let dir = repository_with_wrapped_files()?;
    let run = xtask(dir.path(), &["markdown"])?;
    assert_eq!(run.code, 1, "{}", run.stderr);
    assert_eq!(run.stdout, "");
    assert!(
        run.stderr
            .contains("tracked.md:4: 段落の途中で改行されている\n")
    );
    assert!(
        run.stderr
            .contains("untracked.md:2: 段落の途中で改行されている\n")
    );
    assert!(!run.stderr.contains("ignored.md"));
    assert!(run.stderr.contains("\n2箇所。"));
    assert!(run.stderr.contains("`cargo xtask markdown --fix`"));
    assert_eq!(
        read(dir.path(), "tracked.md")?,
        "# t\n\nfirst line\nsecond line\n"
    );
    Ok(())
}

#[test]
fn fix_joins_the_lines_and_the_check_then_passes() -> Checked {
    let dir = repository_with_wrapped_files()?;
    let fixed = xtask(dir.path(), &["markdown", "--fix"])?;
    assert_eq!(fixed.code, 0, "{}", fixed.stderr);
    assert_eq!(fixed.stderr, "");
    assert_eq!(
        fixed.stdout,
        "tracked.md: 1行を前の行へ結合した\nuntracked.md: 1行を前の行へ結合した\n"
    );
    assert_eq!(
        read(dir.path(), "tracked.md")?,
        "# t\n\nfirst line second line\n"
    );
    assert_eq!(read(dir.path(), "untracked.md")?, "日本語の文章。\n");
    assert_eq!(read(dir.path(), "ignored.md")?, "a\nb\n");

    let checked = xtask(dir.path(), &["markdown"])?;
    assert_eq!(checked.code, 0, "{}", checked.stderr);
    assert_eq!(checked.stdout, "");
    assert_eq!(checked.stderr, "");
    Ok(())
}

#[test]
fn explicit_paths_are_checked_without_git() -> Checked {
    let dir = tempfile::tempdir().required_because("temporary directory")?;
    write(dir.path(), "wrapped.md", "a\nb\n")?;
    write(dir.path(), "clean.md", "a b\n")?;
    let run = xtask(dir.path(), &["markdown", "clean.md", "wrapped.md"])?;
    assert_eq!(run.code, 1, "{}", run.stderr);
    assert!(
        run.stderr
            .starts_with("wrapped.md:2: 段落の途中で改行されている\n")
    );
    assert!(run.stderr.contains("\n1箇所。"));

    let clean = xtask(dir.path(), &["markdown", "clean.md"])?;
    assert_eq!(clean.code, 0, "{}", clean.stderr);
    assert_eq!(clean.stderr, "");
    Ok(())
}

#[test]
fn outside_a_repository_the_listing_fails_with_an_error() -> Checked {
    let dir = tempfile::tempdir().required_because("temporary directory")?;
    let run = xtask(dir.path(), &["markdown"])?;
    assert_eq!(run.code, 1, "{}", run.stderr);
    assert!(run.stderr.starts_with("xtask: `git ls-files`が失敗した: "));
    Ok(())
}

#[test]
fn a_file_that_is_not_utf8_is_an_error() -> Checked {
    let dir = tempfile::tempdir().required_because("temporary directory")?;
    write(dir.path(), "binary.md", [0xff, 0xfe])?;
    let run = xtask(dir.path(), &["markdown", "binary.md"])?;
    assert_eq!(run.code, 1, "{}", run.stderr);
    assert!(run.stderr.starts_with("xtask: binary.md: "));
    Ok(())
}

#[test]
fn a_missing_or_unknown_task_prints_the_usage() -> Checked {
    let dir = tempfile::tempdir().required_because("temporary directory")?;
    for arguments in [&[][..], &["nonsense"][..]] {
        let run = xtask(dir.path(), arguments)?;
        assert_eq!(run.code, 2);
        assert_eq!(run.stdout, "");
        assert_eq!(
            run.stderr,
            "usage: cargo xtask markdown [--fix] [FILE...]\n"
        );
    }
    Ok(())
}
