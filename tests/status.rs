//! `status`が診断を始める前に止まる実行の契約。
//!
//! `status`はhostも案件も変えない読み取りであり、答えられなかったことも結果として示す。
//! ただし表を作る前に決まらないものが2つある。表示言語と、診断する案件そのものである。
//! どちらも欠けたまま進めず、errorを見せて終わる。host toolへは一切問い合わせないため、
//! `PATH`を空にしたまま実行できる。

mod outcome;

use outcome::{Checked, Required};

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

/// 実行結果。
struct Run {
    stdout: String,
    stderr: String,
    code: i32,
}

/// `一時HOME`で`sbxm`を実行する。
///
/// host toolの有無で結果が揺れないよう`PATH`は空にし、表示言語はconfigとargvだけで
/// 決まるようにlocale環境変数を外す。
fn sbxm(home: &Path, arguments: &[&str]) -> Checked<Run> {
    let output = Command::new(env!("CARGO_BIN_EXE_sbxm"))
        .args(arguments)
        .current_dir(home)
        .env("HOME", home)
        .env("LC_ALL", "C")
        .env_remove("LC_MESSAGES")
        .env_remove("LANG")
        .env("PATH", "")
        .output()
        .required_because("sbxm runs")?;
    Ok(Run {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        code: output
            .status
            .code()
            .required_because("the process exits normally")?,
    })
}

fn temp_home() -> Checked<tempfile::TempDir> {
    tempfile::tempdir().required_because("temporary home")
}

/// `~/.sbxm/config.yaml`を、指定したmodeで書く。
fn write_config(home: &Path, body: &str, mode: u32) -> Checked {
    let dir = home.join(".sbxm");
    std::fs::create_dir_all(&dir).required_because("create ~/.sbxm")?;
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))
        .required_because("mode")?;
    let path = dir.join("config.yaml");
    std::fs::write(&path, body).required_because("write config")?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode))
        .required_because("mode")?;
    Ok(())
}

#[test]
fn a_configuration_that_cannot_be_trusted_stops_the_global_report_before_any_row() -> Checked {
    let home = temp_home()?;
    // 他人が読めるconfigは、書かれている値も信用できない。表示言語もそこから決められない。
    write_config(home.path(), "version: 1\nlanguage: ja\n", 0o644)?;

    let run = sbxm(home.path(), &["status", "--global"])?;

    assert_eq!(run.code, 1, "{}{}", run.stdout, run.stderr);
    assert!(
        run.stderr.contains("config-permission-too-open"),
        "{}",
        run.stderr
    );
    // 診断の途中経過を見せない。表は1行も出ない。
    assert!(run.stdout.is_empty(), "{}", run.stdout);
    Ok(())
}

#[test]
fn a_configuration_that_cannot_be_trusted_stops_a_project_report_before_the_lookup() -> Checked {
    let home = temp_home()?;
    write_config(home.path(), "version: 1\nlanguage: ja\n", 0o644)?;

    let run = sbxm(home.path(), &["status", "example-org/example-repo"])?;

    assert_eq!(run.code, 1, "{}{}", run.stdout, run.stderr);
    assert!(
        run.stderr.contains("config-permission-too-open"),
        "{}",
        run.stderr
    );
    // 案件を引く前に止まるため、未登録であることは伝えない。
    assert!(
        !run.stderr.contains("project-not-managed"),
        "{}",
        run.stderr
    );
    assert!(run.stdout.is_empty(), "{}", run.stdout);
    Ok(())
}

#[test]
fn a_project_that_was_never_added_is_refused_rather_than_shown_as_an_empty_report() -> Checked {
    let home = temp_home()?;
    write_config(home.path(), "version: 1\nlanguage: en\n", 0o600)?;

    let run = sbxm(home.path(), &["status", "example-org/example-repo"])?;

    assert_eq!(run.code, 1, "{}{}", run.stdout, run.stderr);
    assert!(run.stderr.contains("project-not-managed"), "{}", run.stderr);
    assert!(
        run.stderr.contains("sbxm add"),
        "the way to manage it is shown: {}",
        run.stderr
    );
    // 読めなかった案件の表を、空の観測結果として作らない。
    assert!(run.stdout.is_empty(), "{}", run.stdout);
    Ok(())
}

#[test]
fn the_saved_language_decides_the_global_report_when_no_flag_asks_for_one() -> Checked {
    let home = temp_home()?;
    write_config(home.path(), "version: 1\nlanguage: ja\n", 0o600)?;

    let run = sbxm(home.path(), &["status", "--global"])?;

    // 保存済みの`language`は、`--lang`が無い実行の表示言語である。
    assert!(run.stdout.contains("状態 (STATUS)"), "{}", run.stdout);
    // 状態値そのものは訳さない。
    assert!(run.stdout.contains("missing"), "{}", run.stdout);
    Ok(())
}
