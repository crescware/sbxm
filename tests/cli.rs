//! CLIの公開契約。
//!
//! exit codeは`0`、`1`、`130`だけを使う。CLI parserを含む内部libraryの既定exit codeを
//! 公開契約へ透過しない。helpとusageは選択したlocaleで生成する。

mod outcome;

use outcome::{Checked, Required};

use std::path::Path;
use std::process::{Command, Output};

const COMMANDS: [&str; 9] = [
    "add", "apply", "prepare", "rebuild", "open", "stop", "ls", "status", "destroy",
];

/// 実行結果。
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

/// `一時HOMEを使ってsbxmを実行する`。
///
/// 実行のたびにHOMEを差し替えるため、利用者のconfigには触れない。
fn sbxm(home: &Path, arguments: &[&str]) -> Checked<Run> {
    sbxm_in(home, home, arguments)
}

/// 親directoryを明示してsbxmを実行する。
///
/// `add`は実行時のcurrent directoryへproject rootを足す。どこで実行したかがそのまま
/// 配置先になるため、testもcwdを明示する。
fn sbxm_in(home: &Path, cwd: &Path, arguments: &[&str]) -> Checked<Run> {
    let output = Command::new(env!("CARGO_BIN_EXE_sbxm"))
        .args(arguments)
        .current_dir(cwd)
        .env("HOME", home)
        // locale決定をtest環境のlocaleへ依存させない。
        .env("LC_ALL", "C")
        .env_remove("LC_MESSAGES")
        .env_remove("LANG")
        // PATHを空にして、host toolの有無でstatusが揺れないようにする。
        .env("PATH", "")
        .output()
        .required_because("sbxm runs")?;
    Run::from(&output)
}

fn temp_home() -> Checked<tempfile::TempDir> {
    tempfile::tempdir().required_because("temporary home")
}

/// hostのGit identityだけに答える`git`を用意し、そのdirectoryを返す。
///
/// hostの値はpromptへ置く候補にしかならないため、名義を宣言する実行はこれを読まない。
/// それでも`git`自体は必要である。cloneには答えないため、実行はcloneで止まる。
fn fake_git(home: &Path) -> Checked<std::path::PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    let bin = home.join("bin");
    std::fs::create_dir_all(&bin).required_because("create the fake bin directory")?;
    let git = bin.join("git");
    std::fs::write(
        &git,
        "#!/bin/sh\n\
         case \"$1 $2 $3 $4\" in\n\
         \"config --global --get-all user.name\") echo 'Example User'; exit 0;;\n\
         \"config --global --get-all user.email\") echo 'user@example.com'; exit 0;;\n\
         esac\n\
         exit 1\n",
    )
    .required_because("write the fake git")?;
    std::fs::set_permissions(&git, std::fs::Permissions::from_mode(0o755))
        .required_because("mode")?;
    Ok(bin)
}

/// hostのGit identityだけに答える`git``を置いたPATHでsbxmを実行する`。
fn sbxm_with_git(home: &Path, cwd: &Path, arguments: &[&str]) -> Checked<Run> {
    let bin = fake_git(home)?;
    let output = Command::new(env!("CARGO_BIN_EXE_sbxm"))
        .args(arguments)
        .current_dir(cwd)
        .env("HOME", home)
        .env("LC_ALL", "C")
        .env_remove("LC_MESSAGES")
        .env_remove("LANG")
        .env("PATH", &bin)
        .output()
        .required_because("sbxm runs")?;
    Run::from(&output)
}

/// 同梱するresourceのtag。言語を増やしてもtestを編集しない。
fn locale_tags() -> Checked<Vec<String>> {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("locales");
    let mut tags: Vec<String> = Vec::new();
    for entry in
        std::fs::read_dir(&directory).required_because("the locales directory is readable")?
    {
        let path = entry.required_because("directory entry")?.path();
        if path.extension().is_some_and(|extension| extension == "ftl")
            && let Some(tag) = path.file_stem().and_then(|stem| stem.to_str())
        {
            tags.push(tag.to_string());
        }
    }
    tags.sort();
    assert!(!tags.is_empty(), "no FTL resource was found");
    Ok(tags)
}

fn write_config(home: &Path, language: &str) -> Checked {
    use std::os::unix::fs::PermissionsExt;
    let dir = home.join(".sbxm");
    std::fs::create_dir_all(&dir).required_because("create ~/.sbxm")?;
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))
        .required_because("mode")?;
    let path = dir.join("config.yaml");
    std::fs::write(&path, format!("version: 1\nlanguage: {language}\n"))
        .required_because("write config")?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
        .required_because("mode")?;
    Ok(())
}

/// helpは全localeで、全commandについて成功する。
///
/// 出力の中身はresourceが正本であり、ここでは複製しない。構造の契約は
/// `tests/snapshots/cli-surface.txt`が、slotがresourceで埋まることは`src/cli.rs`のtestが持つ。
#[test]
fn help_exits_with_zero_in_every_locale_for_every_command() -> Checked {
    let home = temp_home()?;
    for tag in locale_tags()? {
        let run = sbxm(home.path(), &["--lang", &tag, "--help"])?;
        assert_eq!(run.code, 0, "{tag}: --help must succeed: {}", run.stderr);
        assert!(run.stderr.is_empty(), "{tag}: {}", run.stderr);
        assert!(
            !run.stdout.trim().is_empty(),
            "{tag}: help must not be empty"
        );

        for command in COMMANDS {
            let run = sbxm(home.path(), &["--lang", &tag, command, "--help"])?;
            assert_eq!(
                run.code, 0,
                "{tag}: {command} --help must succeed: {}",
                run.stderr
            );
            assert!(run.stderr.is_empty(), "{tag}: {command}: {}", run.stderr);
            assert!(
                !run.stdout.trim().is_empty(),
                "{tag}: {command} help must not be empty"
            );
        }
    }
    Ok(())
}

/// 同梱したresourceは、そのまま`--lang`が受け付ける値になる。
///
/// 逆向き（登録済みlocaleのresourceが存在すること）は`include_str!`がbuild時に保証する。
#[test]
fn every_shipped_resource_is_an_accepted_language() -> Checked {
    let home = temp_home()?;
    for tag in locale_tags()? {
        let run = sbxm(home.path(), &["--lang", &tag, "--help"])?;
        assert_eq!(
            run.code, 0,
            "{tag} ships as a resource but is not accepted: {}",
            run.stderr
        );
    }
    Ok(())
}

#[test]
fn help_is_written_to_stdout_and_never_mixed_with_diagnostics() -> Checked {
    let home = temp_home()?;
    let run = sbxm(home.path(), &["--help"])?;
    assert!(run.stdout.contains("Usage:"), "{}", run.stdout);
    assert!(run.stderr.is_empty(), "{}", run.stderr);
    Ok(())
}

#[test]
fn version_prints_the_version_and_exits_with_zero() -> Checked {
    let home = temp_home()?;
    let run = sbxm(home.path(), &["--version"])?;
    assert_eq!(run.code, 0);
    assert_eq!(
        run.stdout.trim(),
        format!("sbxm {}", env!("CARGO_PKG_VERSION"))
    );
    Ok(())
}

/// shell localeだけを差し替えてsbxmを実行する。
///
/// `LC_ALL`を唯一の手がかりにするため、後ろの2つは取り除いた状態で渡す。
fn sbxm_with_shell_locale(home: &Path, lc_all: &str, arguments: &[&str]) -> Checked<Run> {
    let output = Command::new(env!("CARGO_BIN_EXE_sbxm"))
        .args(arguments)
        .current_dir(home)
        .env("HOME", home)
        .env("LC_ALL", lc_all)
        .env_remove("LC_MESSAGES")
        .env_remove("LANG")
        .env("PATH", "")
        .output()
        .required_because("sbxm runs")?;
    Run::from(&output)
}

#[test]
fn a_shell_locale_that_names_no_shipped_language_leaves_the_output_in_the_source_language()
-> Checked {
    let home = temp_home()?;

    // 実在しないtagを使う。将来どの言語を足してもこのtestは意味を保つ。
    let unknown = sbxm_with_shell_locale(home.path(), "zz_ZZ.UTF-8", &["--help"])?;
    assert_eq!(unknown.code, 0, "{}", unknown.stderr);
    assert!(unknown.stdout.contains("Usage:"), "{}", unknown.stdout);

    // 同じ実行が、読めるtagならその言語を選ぶ。shell localeを見ていること自体は、
    // 2回の実行の差が示す。
    let japanese = sbxm_with_shell_locale(home.path(), "ja_JP.UTF-8", &["--help"])?;
    assert!(
        japanese.stdout.contains("使い方 (Usage):"),
        "{}",
        japanese.stdout
    );
    Ok(())
}

#[test]
fn the_language_option_is_read_before_the_configuration() -> Checked {
    let home = temp_home()?;
    write_config(home.path(), "ja")?;

    // configがjaでも、`--lang en`が優先される。
    let run = sbxm(home.path(), &["--lang", "en", "--help"])?;
    assert!(run.stdout.contains("Usage:"), "{}", run.stdout);

    // `--lang`がなければconfigのlanguageを使う。
    let run = sbxm(home.path(), &["--help"])?;
    assert!(run.stdout.contains("使い方 (Usage):"), "{}", run.stdout);
    Ok(())
}

#[test]
fn a_broken_configuration_does_not_stop_help_from_being_shown() -> Checked {
    use std::os::unix::fs::PermissionsExt;
    let home = temp_home()?;
    let dir = home.path().join(".sbxm");
    std::fs::create_dir_all(&dir).required_because("the fixture directory is created")?;
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))
        .required_because("the fixture permissions are applied")?;
    std::fs::write(dir.join("config.yaml"), "version: 99\n")
        .required_because("the fixture file is written")?;
    std::fs::set_permissions(
        dir.join("config.yaml"),
        std::fs::Permissions::from_mode(0o600),
    )
    .required_because("the fixture permissions are applied")?;

    let run = sbxm(home.path(), &["--help"])?;
    assert_eq!(run.code, 0, "help must not fail because of a broken config");
    assert!(run.stdout.contains("Usage:"), "{}", run.stdout);

    // 通常commandは、同じconfig不正をparse成功後のconfig loadで診断する。
    let run = sbxm(home.path(), &["ls"])?;
    assert_eq!(run.code, 1);
    assert!(
        run.stderr.contains("config-unknown-version"),
        "{}",
        run.stderr
    );
    Ok(())
}

#[test]
fn an_unsupported_language_is_reported_before_any_configuration_error() -> Checked {
    use std::os::unix::fs::PermissionsExt;

    let home = temp_home()?;
    // 実在しないtagを使う。将来どの言語を足してもこのtestは意味を保つ。
    let run = sbxm(home.path(), &["--lang", "zz", "ls"])?;
    assert_eq!(run.code, 1);
    assert!(run.stderr.contains("invalid-lang"), "{}", run.stderr);
    assert!(run.stdout.is_empty(), "{}", run.stdout);

    // 壊れたconfigはparse errorを覆い隠さない。
    let dir = home.path().join(".sbxm");
    std::fs::create_dir_all(&dir).required_because("the fixture directory is created")?;
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))
        .required_because("the fixture permissions are applied")?;
    std::fs::write(dir.join("config.yaml"), "version: 99\n")
        .required_because("the fixture file is written")?;
    std::fs::set_permissions(
        dir.join("config.yaml"),
        std::fs::Permissions::from_mode(0o600),
    )
    .required_because("the fixture permissions are applied")?;

    let run = sbxm(home.path(), &["--lang", "zz", "ls"])?;
    assert_eq!(run.code, 1);
    assert!(run.stderr.contains("invalid-lang"), "{}", run.stderr);
    assert!(
        !run.stderr.contains("config-unknown-version"),
        "the broken config must not be diagnosed instead: {}",
        run.stderr
    );

    // 読めるconfigがあっても、`--lang`の不正はそのまま失敗する。診断の本文は
    // configが宣言したlocaleで出す。resourceを複製せず、2回の実行の差で見る。
    write_config(home.path(), "en")?;
    let english = sbxm(home.path(), &["--lang", "zz", "ls"])?;
    assert_eq!(english.code, 1);
    assert!(
        english.stderr.contains("invalid-lang"),
        "{}",
        english.stderr
    );

    write_config(home.path(), "ja")?;
    let japanese = sbxm(home.path(), &["--lang", "zz", "ls"])?;
    assert_eq!(japanese.code, 1);
    assert!(
        japanese.stderr.contains("invalid-lang"),
        "{}",
        japanese.stderr
    );
    assert_ne!(
        english.stderr, japanese.stderr,
        "the diagnostic must be rendered in the locale the config declares"
    );
    Ok(())
}

#[test]
fn parser_failures_map_to_exit_code_one() -> Checked {
    let home = temp_home()?;
    for arguments in [
        vec!["--nope"],
        vec!["nope"],
        vec![],
        vec!["add"],
        vec!["add", "owner/repo"],
        vec!["add", "git@github.com:owner/repo.git", "--worktrees", "0"],
        vec!["add", "git@github.com:owner/repo.git", "--worktrees", "2"],
        vec!["status"],
        vec!["status", "--global", "owner/repo"],
    ] {
        let run = sbxm(home.path(), &arguments)?;
        assert_eq!(
            run.code, 1,
            "{arguments:?} must exit with 1, got {}: {}",
            run.code, run.stderr
        );
        // markerは色なしでも残る。severityを色だけに委ねない。
        assert!(
            run.stderr.starts_with("\u{d7} error: "),
            "{arguments:?}: {}",
            run.stderr
        );
    }
    Ok(())
}

#[test]
fn diagnostics_name_a_stable_error_id() -> Checked {
    let home = temp_home()?;
    for (arguments, expected) in [
        (vec!["nope"], "unknown-subcommand"),
        (vec!["ls", "--nope"], "unknown-argument"),
        (vec!["status"], "status-scope-required"),
        (
            vec!["add", "git@github.com:owner/repo.git", "--worktrees", "33"],
            "worktrees-out-of-range",
        ),
        (
            vec!["add", "git@github.com:owner/repo.git", "--worktrees", "2"],
            "worktrees-require-detach",
        ),
        (vec!["add", "not-a-project"], "invalid-clone-url"),
        (
            vec!["add", "git@github.com:not a project/repo.git"],
            "invalid-project-id",
        ),
        (vec!["apply", "owner/repo"], "apply-scope-required"),
    ] {
        let run = sbxm(home.path(), &arguments)?;
        assert!(
            run.stderr.contains(expected),
            "{arguments:?} should report {expected}: {}",
            run.stderr
        );
    }
    Ok(())
}

#[test]
fn a_non_interactive_run_that_omits_the_target_is_a_usage_error() -> Checked {
    let home = temp_home()?;
    for command in ["open", "stop", "destroy"] {
        let run = sbxm(home.path(), &[command])?;
        assert_eq!(run.code, 1, "{command}: {}", run.stderr);
        assert!(
            run.stderr.contains("project-argument-required"),
            "{command}: {}",
            run.stderr
        );
    }
    Ok(())
}

#[test]
fn a_missing_configuration_is_the_defaults_rather_than_a_reason_to_stop() -> Checked {
    let home = temp_home()?;
    // configが無くても、commandはhost toolまで進む。
    let run = sbxm(home.path(), &["ls"])?;
    assert_eq!(run.code, 1);
    assert!(
        run.stderr.contains("external-command-not-found"),
        "{}",
        run.stderr
    );
    assert!(
        !home.path().join(".sbxm").exists(),
        "a read-only command must not create the global state directory"
    );
    Ok(())
}

#[test]
fn add_registers_the_project_before_it_reaches_the_host_tools() -> Checked {
    use std::os::unix::fs::PermissionsExt;
    let home = temp_home()?;
    let base = home.path().join("Projects");
    std::fs::create_dir_all(&base).required_because("the fixture directory is created")?;
    write_config(home.path(), "en")?;

    let run = sbxm_with_git(
        home.path(),
        &base,
        &[
            "--lang",
            "en",
            "add",
            "git@github.com:Example-Org/Example-Repo.git",
            "--git-user-name",
            "Example User",
            "--git-user-email",
            "user@example.com",
        ],
    )?;
    assert_eq!(run.code, 1, "{}", run.stderr);
    assert!(
        run.stderr.contains("external-command-failed"),
        "the run stops at the clone it cannot make: {}",
        run.stderr
    );

    // 登録そのものは終わっているため、案件directoryが残る。owner名のdirectoryは作らない。
    let root = base.join("example-repo.project");
    assert!(
        !base.join("example-org").exists(),
        "the project root sits directly under the parent directory"
    );
    let metadata = root.join(".sbxm").join("project.yaml");
    assert!(metadata.is_file(), "the project is registered");
    assert!(root.join(".sbxm").join("Dockerfile").is_file());
    assert!(root.join(".sbxm").join(".cache").is_dir());
    assert_eq!(
        std::fs::metadata(root.join(".sbxm"))
            .required_because("the path sbxm created is observable")?
            .permissions()
            .mode()
            & 0o777,
        0o700
    );

    // 索引は`~/.sbxm/registry.yaml`が持ち、project rootを絶対pathで指す。
    let registry = home.path().join(".sbxm").join("registry.yaml");
    let index =
        std::fs::read_to_string(&registry).required_because("the file sbxm wrote is readable")?;
    assert!(index.contains("version: 1"), "{index}");
    assert!(
        index.contains("canonical_id: example-org/example-repo"),
        "{index}"
    );
    assert!(
        index.contains(&format!("project_root: {}", root.display())),
        "{index}"
    );
    assert!(
        index.contains("clone_url: git@github.com:Example-Org/Example-Repo.git"),
        "{index}"
    );
    assert_eq!(
        std::fs::metadata(&registry)
            .required_because("the path sbxm created is observable")?
            .permissions()
            .mode()
            & 0o777,
        0o600
    );

    let written =
        std::fs::read_to_string(&metadata).required_because("the file sbxm wrote is readable")?;
    assert!(
        written.contains("canonical_id: example-org/example-repo"),
        "{written}"
    );
    assert!(written.contains("owner: Example-Org"), "{written}");
    assert!(written.contains("provider: github"), "{written}");
    assert!(written.contains("clone_transport: ssh"), "{written}");
    assert!(
        written.contains("clone_url: git@github.com:Example-Org/Example-Repo.git"),
        "{written}"
    );
    assert!(written.contains("mode: attached"), "{written}");
    Ok(())
}

#[test]
fn ls_needs_the_sandbox_runtime_before_it_can_answer() -> Checked {
    let home = temp_home()?;
    let base = home.path().join("Projects");
    std::fs::create_dir_all(&base).required_because("the fixture directory is created")?;
    write_config(home.path(), "en")?;

    // 一覧はSandbox runtimeの状態から作るため、読めなければ何も出さない。
    let run = sbxm(home.path(), &["--lang", "en", "ls"])?;
    assert_eq!(run.code, 1, "{}", run.stdout);
    assert!(run.stdout.is_empty(), "no partial listing: {}", run.stdout);
    assert!(
        run.stderr.contains("external-command-not-found"),
        "{}",
        run.stderr
    );
    Ok(())
}

/// 案件を登録した状態のHOMEを作る。
///
/// `add`はhost toolに到達した時点で止まるが、登録そのものは終わっている。
fn home_with_project(project: &str) -> Checked<(tempfile::TempDir, std::path::PathBuf)> {
    let home = temp_home()?;
    let base = home.path().join("Projects");
    std::fs::create_dir_all(&base).required_because("the fixture directory is created")?;
    write_config(home.path(), "en")?;
    let url = format!("git@github.com:{project}.git");
    let run = sbxm_with_git(
        home.path(),
        &base,
        &[
            "--lang",
            "en",
            "add",
            &url,
            "--git-user-name",
            "Example User",
            "--git-user-email",
            "user@example.com",
        ],
    )?;
    assert_eq!(run.code, 1, "{}", run.stderr);
    Ok((home, base))
}

/// 表の1列目を、header行を除いて取り出す。
fn first_column(table: &str) -> Vec<String> {
    table
        .lines()
        .skip(1)
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            line.split("  ")
                .next()
                .unwrap_or_default()
                .trim()
                .to_string()
        })
        .collect()
}

#[test]
fn project_status_keeps_the_items_it_could_read_and_names_the_global_command() -> Checked {
    let (home, _base) = home_with_project("owner/repo")?;

    // host toolが無い環境でも、取得できた項目は後続検査の失敗にかかわらず表示する。
    let run = sbxm(home.path(), &["--lang", "en", "status", "owner/repo"])?;
    assert_eq!(run.code, 1, "{}{}", run.stdout, run.stderr);

    let (project, worktrees) = run
        .stdout
        .split_once("\nWORKTREES\n")
        .required_because("both sections are shown, even with nothing to list")?;
    assert!(project.starts_with("PROJECT\n"), "{}", run.stdout);
    assert_eq!(
        first_column(&format!("_\n{}", project.trim_start_matches("PROJECT\n"))),
        vec![
            "Project",
            "Metadata",
            "Project root",
            "Host clone",
            "Dockerfile",
            "Image",
            "Template archive",
            "Sandbox",
            "Workspace",
            "GitHub secret",
            "Bare repository",
            "Worktrees",
            "SSH Agent",
        ],
        "{}",
        run.stdout
    );
    // worktreeが1本もないことも観測結果である。列だけの表ではなく一行で示す。
    assert_eq!(
        worktrees.lines().next(),
        Some("  No managed worktree was observed."),
        "{}",
        run.stdout
    );

    // 観測できなかった項目を、Sandboxが無いことへ丸めない。
    assert!(
        !run.stdout.contains("not-applicable"),
        "an unread state is not the same as an absent sandbox: {}",
        run.stdout
    );
    for id in ["global-scope-unobservable", "sbxm status --global"] {
        assert!(run.stderr.contains(id), "{}", run.stderr);
    }
    Ok(())
}

#[test]
fn the_japanese_project_status_translates_the_labels_and_keeps_the_values() -> Checked {
    let (home, _base) = home_with_project("owner/repo")?;

    let run = sbxm(home.path(), &["--lang", "ja", "status", "owner/repo"])?;
    assert_eq!(run.code, 1, "{}{}", run.stdout, run.stderr);
    // section名と項目名は訳し、状態値は訳さない。
    assert!(run.stdout.contains("案件 (PROJECT)"), "{}", run.stdout);
    assert!(run.stdout.contains("mismatch"), "{}", run.stdout);
    // 凡例はSandboxの状態を説明し、host serviceの説明を流用しない。
    assert!(!run.stdout.contains("service"), "{}", run.stdout);
    Ok(())
}

#[test]
fn apply_refuses_a_project_that_was_never_added() -> Checked {
    let home = temp_home()?;
    let base = home.path().join("Projects");
    std::fs::create_dir_all(&base).required_because("the fixture directory is created")?;
    write_config(home.path(), "en")?;

    let run = sbxm(
        home.path(),
        &["--lang", "en", "apply", "--files", "owner/repo"],
    )?;
    assert_eq!(run.code, 1);
    assert!(run.stderr.contains("project-not-managed"), "{}", run.stderr);
    assert!(
        run.stderr.contains("sbxm add <github-clone-url>"),
        "{}",
        run.stderr
    );
    Ok(())
}

/// 案件を引数で指定するcommand。`apply`は指定の形が違うため別に確かめる。
const PROJECT_COMMANDS: [&str; 5] = ["prepare", "rebuild", "open", "stop", "destroy"];

#[test]
fn every_command_that_targets_a_project_refuses_one_that_was_never_added() -> Checked {
    let home = temp_home()?;
    let base = home.path().join("Projects");
    std::fs::create_dir_all(&base).required_because("the fixture directory is created")?;
    write_config(home.path(), "en")?;

    // 対象が決まる前にhostの状態へ触れない。host toolが1つも無い環境でも、
    // 未登録であることだけを理由として断る。
    for command in PROJECT_COMMANDS {
        let run = sbxm(home.path(), &["--lang", "en", command, "owner/repo"])?;
        assert_eq!(run.code, 1, "{command}: {}", run.stderr);
        assert!(
            run.stdout.is_empty(),
            "{command} wrote a result for a project it refused: {}",
            run.stdout
        );
        assert!(
            run.stderr.contains("project-not-managed"),
            "{command}: {}",
            run.stderr
        );
    }
    Ok(())
}

#[test]
fn a_registered_project_still_needs_the_sandbox_runtime() -> Checked {
    let (home, _base) = home_with_project("owner/repo")?;

    // 登録は済んでいるため対象は決まる。その先はhost toolに届かず、どのcommandも
    // 結果を出さずに止まる。
    for command in PROJECT_COMMANDS {
        let run = sbxm(home.path(), &["--lang", "en", command, "owner/repo"])?;
        assert_eq!(run.code, 1, "{command}: {}", run.stderr);
        assert!(
            run.stdout.is_empty(),
            "{command} wrote a result it could not observe: {}",
            run.stdout
        );
        assert!(
            !run.stderr.contains("project-not-managed"),
            "{command} reached the project it was given: {}",
            run.stderr
        );
        assert!(
            run.stderr.contains("external-command-not-found"),
            "{command}: {}",
            run.stderr
        );
    }
    Ok(())
}

#[test]
fn a_run_that_cannot_prompt_neither_asks_for_a_language_nor_saves_one() -> Checked {
    let home = temp_home()?;
    let base = home.path().join("Projects");
    std::fs::create_dir_all(&base).required_because("the fixture directory is created")?;

    // 非対話の`add`はpromptを出さず、言語も名義も永続化しない。`--lang`と同じく、
    // command lineの宣言はそのprocessだけのoverrideである。
    let run = sbxm_with_git(
        home.path(),
        &base,
        &[
            "--lang",
            "ja",
            "add",
            "git@github.com:Example-Org/Example-Repo.git",
            "--git-user-name",
            "Example User",
            "--git-user-email",
            "user@example.com",
        ],
    )?;
    assert_eq!(run.code, 1, "{}", run.stderr);
    assert!(
        !home.path().join(".sbxm").join("config.yaml").exists(),
        "neither the language nor the identity becomes a side effect of an option"
    );
    // それでも登録は進む。registryは作られる。
    assert!(home.path().join(".sbxm").join("registry.yaml").is_file());
    Ok(())
}

#[test]
fn a_non_interactive_add_without_an_identity_refuses_before_creating_anything() -> Checked {
    let home = temp_home()?;
    let base = home.path().join("Projects");
    std::fs::create_dir_all(&base).required_because("the fixture directory is created")?;

    // hostは名義を宣言しているが、それは候補であって利用者の意図ではない。訊く先も
    // 保存された既定も宣言も無い実行は、何も作らずに終わる。
    let run = sbxm_with_git(
        home.path(),
        &base,
        &[
            "--lang",
            "en",
            "add",
            "git@github.com:Example-Org/Example-Repo.git",
        ],
    )?;
    assert_eq!(run.code, 1, "{}", run.stderr);
    assert!(
        run.stderr.contains("git-identity-undecidable"),
        "{}",
        run.stderr
    );
    assert!(
        !home.path().join(".sbxm").exists(),
        "no global state is created: {}",
        run.stderr
    );
    assert!(
        !base.join("example-repo.project").exists(),
        "no project directory is created"
    );
    Ok(())
}

/// 既定の名義を保存済みのconfigを置く。
fn write_config_with_identity(home: &Path, name: &str, email: &str) -> Checked {
    use std::os::unix::fs::PermissionsExt;
    let dir = home.join(".sbxm");
    std::fs::create_dir_all(&dir).required_because("create ~/.sbxm")?;
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))
        .required_because("mode")?;
    let path = dir.join("config.yaml");
    std::fs::write(
        &path,
        format!("version: 1\nlanguage: en\ngit_user_name: {name}\ngit_user_email: {email}\n"),
    )
    .required_because("write config")?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
        .required_because("mode")?;
    Ok(())
}

#[test]
fn half_a_declared_identity_is_refused_whatever_else_could_have_decided_it() -> Checked {
    // 片方だけの宣言はCLI parseの段階で止まる。保存済みの既定があっても、残りの半分を
    // そこから補完しない。
    for saved in [false, true] {
        let home = temp_home()?;
        let base = home.path().join("Projects");
        std::fs::create_dir_all(&base).required_because("the fixture directory is created")?;
        if saved {
            write_config_with_identity(home.path(), "Saved User", "saved@example.com")?;
        }

        for option in [
            ("--git-user-name", "Example User"),
            ("--git-user-email", "user@example.com"),
        ] {
            let run = sbxm_with_git(
                home.path(),
                &base,
                &[
                    "--lang",
                    "en",
                    "add",
                    "git@github.com:Example-Org/Example-Repo.git",
                    option.0,
                    option.1,
                ],
            )?;
            assert_eq!(run.code, 1, "{}", run.stderr);
            assert!(
                run.stderr.contains("git-identity-incomplete"),
                "{} alone must be refused with a saved default of {saved}: {}",
                option.0,
                run.stderr
            );
            assert!(
                !home.path().join(".sbxm").join("registry.yaml").exists(),
                "nothing is registered before the declaration is complete"
            );
            assert!(
                !base.join("example-repo.project").exists(),
                "no project directory is created"
            );
        }

        if !saved {
            assert!(
                !home.path().join(".sbxm").exists(),
                "with nothing saved, no global state is created either"
            );
        }
    }
    Ok(())
}

#[test]
fn init_is_not_part_of_the_published_surface() -> Checked {
    let home = temp_home()?;
    let run = sbxm(home.path(), &["init"])?;
    assert_eq!(run.code, 1);
    assert!(run.stderr.contains("unknown-subcommand"), "{}", run.stderr);
    assert!(
        !home.path().join(".sbxm").exists(),
        "an unknown command creates nothing"
    );

    let help = sbxm(home.path(), &["--lang", "en", "--help"])?;
    assert_eq!(help.code, 0, "{}", help.stderr);
    assert!(!help.stdout.contains("init"), "{}", help.stdout);
    Ok(())
}

#[test]
fn global_status_answers_without_visiting_the_registered_projects() -> Checked {
    use std::os::unix::fs::PermissionsExt;
    let home = temp_home()?;
    let dir = home.path().join(".sbxm");
    std::fs::create_dir_all(&dir).required_because("the fixture directory is created")?;
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))
        .required_because("the fixture permissions are applied")?;
    // 実在しないproject rootを指すentry。案件を巡回するなら、ここで失敗する。
    std::fs::write(
        dir.join("registry.yaml"),
        "version: 1\nprojects:\n- canonical_id: example-org/example-repo\n  project_root: /nowhere/example-repo.project\n  provider: github\n  clone_transport: ssh\n  clone_url: git@github.com:Example-Org/Example-Repo.git\n",
    )
    .required_because("the fixture file is written")?;
    std::fs::set_permissions(
        dir.join("registry.yaml"),
        std::fs::Permissions::from_mode(0o600),
    )
    .required_because("the fixture permissions are applied")?;

    let run = sbxm(home.path(), &["--lang", "en", "status", "--global"])?;
    let registry_row = run
        .stdout
        .lines()
        .find(|line| line.starts_with("Project registry"))
        .required_because("the registry row is shown")?;
    assert!(registry_row.contains("ready"), "{}", run.stdout);
    assert!(
        !run.stderr.contains("example-repo"),
        "no project is visited: {}",
        run.stderr
    );
    Ok(())
}

#[test]
fn read_only_commands_never_create_a_configuration() -> Checked {
    let home = temp_home()?;
    for arguments in [vec!["ls"], vec!["status", "--global"], vec!["--help"]] {
        let run = sbxm(home.path(), &arguments)?;
        assert!(
            !home.path().join(".sbxm").join("config.yaml").exists(),
            "{arguments:?} created a configuration: {}",
            run.stderr
        );
    }
    Ok(())
}

#[test]
fn global_status_prints_only_the_global_section_and_the_published_columns() -> Checked {
    let home = temp_home()?;
    let run = sbxm(home.path(), &["--lang", "en", "status", "--global"])?;

    let lines: Vec<&str> = run.stdout.lines().collect();
    assert_eq!(lines[0], "GLOBAL");
    assert!(lines[1].starts_with("ITEM"), "{}", lines[1]);
    assert!(lines[1].trim_end().ends_with("STATUS"), "{}", lines[1]);

    let items: Vec<&str> = lines[2..]
        .iter()
        .map(|line| line.split("  ").next().unwrap_or("").trim())
        .collect();
    assert_eq!(
        items,
        vec![
            "Global state",
            "Config",
            "Project registry",
            "Platform",
            "Git",
            "SSH",
            "Docker",
            "Git identity",
            "Docker Sandboxes",
            "Network policy",
            "Daemon",
            "Docker Sandboxes login",
            "Remote SSH",
        ]
    );
    assert!(!run.stdout.contains("PROJECT"), "{}", run.stdout);
    assert!(!run.stdout.contains("WORKTREES"), "{}", run.stdout);
    Ok(())
}

#[test]
fn global_status_reports_every_problem_and_exits_with_one() -> Checked {
    let home = temp_home()?;
    let run = sbxm(home.path(), &["--lang", "en", "status", "--global"])?;

    assert_eq!(run.code, 1, "an incomplete host is not healthy");
    // 取得できた行は後続検査が失敗しても省略しない。
    assert_eq!(run.stdout.lines().count(), 15);
    for id in ["external-command-not-found", "host-command-missing"] {
        assert!(
            run.stderr.contains(id),
            "{id} is missing from: {}",
            run.stderr
        );
    }
    // 詳細は表の列を増やさず、stderrの診断として出す。
    assert!(!run.stdout.contains("error:"), "{}", run.stdout);
    Ok(())
}

#[test]
fn global_status_never_writes_to_the_host() -> Checked {
    let home = temp_home()?;
    sbxm(home.path(), &["status", "--global"])?;
    assert_eq!(
        std::fs::read_dir(home.path())
            .required_because("the home directory is readable")?
            .count(),
        0,
        "a read-only diagnosis must not create anything under HOME"
    );
    Ok(())
}

#[test]
fn the_japanese_mode_adds_a_legend_for_the_values_that_appeared() -> Checked {
    let home = temp_home()?;
    let run = sbxm(home.path(), &["--lang", "ja", "status", "--global"])?;

    let legend = run
        .stdout
        .split_once("状態値の凡例\n")
        .required_because("the legend is its own section")?
        .1;
    // 値は訳さず、説明だけを訳す。
    assert!(legend.contains("error "), "{legend}");
    assert!(
        !legend.contains("running"),
        "values that did not appear must be left out: {legend}"
    );

    // 英語modeには凡例を出さない。
    let english = sbxm(home.path(), &["--lang", "en", "status", "--global"])?;
    assert!(!english.stdout.contains("Legend"), "{}", english.stdout);
    Ok(())
}

#[test]
fn status_values_stay_untranslated_in_the_japanese_mode() -> Checked {
    let home = temp_home()?;
    let run = sbxm(home.path(), &["--lang", "ja", "status", "--global"])?;
    assert!(run.stdout.contains("項目 (ITEM)"), "{}", run.stdout);
    assert!(run.stdout.contains("設定 (Config)"), "{}", run.stdout);
    assert!(run.stdout.contains("missing"), "{}", run.stdout);
    assert!(run.stdout.contains("error"), "{}", run.stdout);
    Ok(())
}
