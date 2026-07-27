//! CLIの公開契約。
//!
//! exit codeは`0`、`1`、`130`だけを使う。CLI parserを含む内部libraryの既定exit codeを
//! 公開契約へ透過しない。helpとusageは選択したlocaleで生成する。

use std::path::Path;
use std::process::{Command, Output};

const COMMANDS: [&str; 9] = [
    "init",
    "add",
    "sync-files",
    "rebuild",
    "open",
    "stop",
    "ls",
    "status",
    "destroy",
];

/// 実行結果。
struct Run {
    stdout: String,
    stderr: String,
    code: i32,
}

impl Run {
    fn from(output: Output) -> Run {
        Run {
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            code: output.status.code().expect("the process exits normally"),
        }
    }
}

/// 一時HOMEを使ってsbxmを実行する。
///
/// 実行のたびにHOMEを差し替えるため、利用者のconfigには触れない。
fn sbxm(home: &Path, arguments: &[&str]) -> Run {
    let output = Command::new(env!("CARGO_BIN_EXE_sbxm"))
        .args(arguments)
        .env("HOME", home)
        // locale決定をtest環境のlocaleへ依存させない。
        .env("LC_ALL", "C")
        .env_remove("LC_MESSAGES")
        .env_remove("LANG")
        // PATHを空にして、host toolの有無でstatusが揺れないようにする。
        .env("PATH", "")
        .output()
        .expect("sbxm runs");
    Run::from(output)
}

fn temp_home() -> tempfile::TempDir {
    tempfile::tempdir().expect("temporary home")
}

/// 同梱するresourceのtag。言語を増やしてもtestを編集しない。
fn locale_tags() -> Vec<String> {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("locales");
    let mut tags: Vec<String> = std::fs::read_dir(&directory)
        .expect("the locales directory is readable")
        .map(|entry| entry.expect("directory entry").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "ftl"))
        .filter_map(|path| {
            path.file_stem()
                .and_then(|stem| stem.to_str())
                .map(|stem| stem.to_string())
        })
        .collect();
    tags.sort();
    assert!(!tags.is_empty(), "no FTL resource was found");
    tags
}

fn write_config(home: &Path, base_path: &Path, language: &str) {
    use std::os::unix::fs::PermissionsExt;
    let dir = home.join(".sbxm");
    std::fs::create_dir_all(&dir).expect("create ~/.sbxm");
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).expect("mode");
    let path = dir.join("config.toml");
    std::fs::write(
        &path,
        format!(
            "version = 1\nlanguage = \"{language}\"\nbase_path = \"{}\"\n\n[git]\nuser_name = \"Example User\"\nuser_email = \"user@example.com\"\n",
            base_path.display()
        ),
    )
    .expect("write config");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).expect("mode");
}

/// helpは全localeで、全commandについて成功する。
///
/// 出力の中身はresourceが正本であり、ここでは複製しない。構造の契約は
/// `tests/snapshots/cli-surface.txt`が、slotがresourceで埋まることは`src/cli.rs`のtestが持つ。
#[test]
fn help_exits_with_zero_in_every_locale_for_every_command() {
    let home = temp_home();
    for tag in locale_tags() {
        let run = sbxm(home.path(), &["--lang", &tag, "--help"]);
        assert_eq!(run.code, 0, "{tag}: --help must succeed: {}", run.stderr);
        assert!(run.stderr.is_empty(), "{tag}: {}", run.stderr);
        assert!(
            !run.stdout.trim().is_empty(),
            "{tag}: help must not be empty"
        );

        for command in COMMANDS {
            let run = sbxm(home.path(), &["--lang", &tag, command, "--help"]);
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
}

/// 同梱したresourceは、そのまま`--lang`が受け付ける値になる。
///
/// 逆向き（登録済みlocaleのresourceが存在すること）は`include_str!`がbuild時に保証する。
#[test]
fn every_shipped_resource_is_an_accepted_language() {
    let home = temp_home();
    for tag in locale_tags() {
        let run = sbxm(home.path(), &["--lang", &tag, "--help"]);
        assert_eq!(
            run.code, 0,
            "{tag} ships as a resource but is not accepted: {}",
            run.stderr
        );
    }
}

#[test]
fn help_is_written_to_stdout_and_never_mixed_with_diagnostics() {
    let home = temp_home();
    let run = sbxm(home.path(), &["--help"]);
    assert!(run.stdout.contains("Usage:"), "{}", run.stdout);
    assert!(run.stderr.is_empty(), "{}", run.stderr);
}

#[test]
fn version_prints_the_version_and_exits_with_zero() {
    let home = temp_home();
    let run = sbxm(home.path(), &["--version"]);
    assert_eq!(run.code, 0);
    assert_eq!(
        run.stdout.trim(),
        format!("sbxm {}", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn the_language_option_is_read_before_the_configuration() {
    let home = temp_home();
    let base = home.path().join("Projects");
    write_config(home.path(), &base, "ja");

    // configがjaでも、`--lang en`が優先される。
    let run = sbxm(home.path(), &["--lang", "en", "--help"]);
    assert!(run.stdout.contains("Usage:"), "{}", run.stdout);

    // `--lang`がなければconfigのlanguageを使う。
    let run = sbxm(home.path(), &["--help"]);
    assert!(run.stdout.contains("使い方 (Usage):"), "{}", run.stdout);
}

#[test]
fn a_broken_configuration_does_not_stop_help_from_being_shown() {
    use std::os::unix::fs::PermissionsExt;
    let home = temp_home();
    let dir = home.path().join(".sbxm");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
    std::fs::write(dir.join("config.toml"), "version = 99\n").unwrap();
    std::fs::set_permissions(
        dir.join("config.toml"),
        std::fs::Permissions::from_mode(0o600),
    )
    .unwrap();

    let run = sbxm(home.path(), &["--help"]);
    assert_eq!(run.code, 0, "help must not fail because of a broken config");
    assert!(run.stdout.contains("Usage:"), "{}", run.stdout);

    // 通常commandは、同じconfig不正をparse成功後のconfig loadで診断する。
    let run = sbxm(home.path(), &["ls"]);
    assert_eq!(run.code, 1);
    assert!(
        run.stderr.contains("config-unknown-version"),
        "{}",
        run.stderr
    );
}

#[test]
fn an_unsupported_language_fails_without_reading_the_configuration() {
    let home = temp_home();
    // 実在しないtagを使う。将来どの言語を足してもこのtestは意味を保つ。
    let run = sbxm(home.path(), &["--lang", "zz", "ls"]);
    assert_eq!(run.code, 1);
    assert!(run.stderr.contains("invalid-lang"), "{}", run.stderr);
    assert!(run.stdout.is_empty(), "{}", run.stdout);
}

#[test]
fn parser_failures_map_to_exit_code_one() {
    let home = temp_home();
    for arguments in [
        vec!["--nope"],
        vec!["nope"],
        vec![],
        vec!["add"],
        vec!["add", "owner/repo", "--worktrees", "0"],
        vec!["add", "owner/repo", "--worktrees", "2"],
        vec!["status"],
        vec!["status", "--global", "owner/repo"],
        vec!["init", "--base-path", "/tmp/projects"],
    ] {
        let run = sbxm(home.path(), &arguments);
        assert_eq!(
            run.code, 1,
            "{arguments:?} must exit with 1, got {}: {}",
            run.code, run.stderr
        );
        assert!(
            run.stderr.starts_with("error: "),
            "{arguments:?}: {}",
            run.stderr
        );
    }
}

#[test]
fn diagnostics_name_a_stable_error_id() {
    let home = temp_home();
    for (arguments, expected) in [
        (vec!["nope"], "unknown-subcommand"),
        (vec!["ls", "--nope"], "unknown-argument"),
        (vec!["status"], "status-scope-required"),
        (
            vec!["add", "owner/repo", "--worktrees", "33"],
            "worktrees-out-of-range",
        ),
        (
            vec!["add", "owner/repo", "--worktrees", "2"],
            "worktrees-require-detach",
        ),
        (
            vec!["init", "--base-path", "/tmp/projects"],
            "init-incomplete-options",
        ),
        (vec!["add", "not-a-project"], "invalid-project-id"),
    ] {
        let run = sbxm(home.path(), &arguments);
        assert!(
            run.stderr.contains(expected),
            "{arguments:?} should report {expected}: {}",
            run.stderr
        );
    }
}

#[test]
fn a_non_interactive_run_that_omits_the_target_is_a_usage_error() {
    let home = temp_home();
    for command in ["open", "stop", "destroy"] {
        let run = sbxm(home.path(), &[command]);
        assert_eq!(run.code, 1, "{command}: {}", run.stderr);
        assert!(
            run.stderr.contains("project-argument-required"),
            "{command}: {}",
            run.stderr
        );
    }
}

#[test]
fn commands_other_than_init_and_global_status_need_a_configuration() {
    let home = temp_home();
    let run = sbxm(home.path(), &["ls"]);
    assert_eq!(run.code, 1);
    assert!(run.stderr.contains("config-missing"), "{}", run.stderr);
    assert!(run.stderr.contains("sbxm init"), "{}", run.stderr);
}

#[test]
fn commands_that_are_not_implemented_yet_say_so_after_validating_their_arguments() {
    let home = temp_home();
    let base = home.path().join("Projects");
    std::fs::create_dir_all(&base).unwrap();
    write_config(home.path(), &base, "en");

    for arguments in [
        vec!["rebuild", "owner/repo"],
        vec!["open", "owner/repo"],
        vec!["stop", "owner/repo"],
        vec!["status", "owner/repo"],
        vec!["destroy", "--force", "owner/repo"],
    ] {
        let run = sbxm(home.path(), &arguments);
        assert_eq!(run.code, 1, "{arguments:?}: {}", run.stderr);
        assert!(
            run.stderr.contains("not-implemented"),
            "{arguments:?}: {}",
            run.stderr
        );
    }
}

/// PATHを空にしているため、外部toolを必要とする工程まで進んだところで止まる。
#[test]
fn add_registers_the_project_before_it_reaches_the_host_tools() {
    use std::os::unix::fs::PermissionsExt;
    let home = temp_home();
    let base = home.path().join("Projects");
    std::fs::create_dir_all(&base).unwrap();
    write_config(home.path(), &base, "en");

    let run = sbxm(
        home.path(),
        &["--lang", "en", "add", "Example-Org/Example-Repo"],
    );
    assert_eq!(run.code, 1, "{}", run.stderr);
    assert!(
        run.stderr.contains("external-command-not-found"),
        "the run stops at the first host tool it needs: {}",
        run.stderr
    );

    // 登録そのものは終わっているため、案件directoryが残る。
    let root = base.join("example-org").join("example-repo.project");
    let metadata = root.join(".sbxm").join("project.toml");
    assert!(metadata.is_file(), "the project is registered");
    assert!(root.join(".sbxm").join("Dockerfile").is_file());
    assert!(root.join(".sbxm").join(".cache").is_dir());
    assert_eq!(
        std::fs::metadata(root.join(".sbxm"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );

    let written = std::fs::read_to_string(&metadata).unwrap();
    assert!(
        written.contains("canonical_id = \"example-org/example-repo\""),
        "{written}"
    );
    assert!(written.contains("owner = \"Example-Org\""), "{written}");
    assert!(written.contains("mode = \"attached\""), "{written}");
}

#[test]
fn ls_needs_the_sandbox_runtime_before_it_can_answer() {
    let home = temp_home();
    let base = home.path().join("Projects");
    std::fs::create_dir_all(&base).unwrap();
    write_config(home.path(), &base, "en");

    // 一覧はSandbox runtimeの状態から作るため、読めなければ何も出さない。
    let run = sbxm(home.path(), &["--lang", "en", "ls"]);
    assert_eq!(run.code, 1, "{}", run.stdout);
    assert!(run.stdout.is_empty(), "no partial listing: {}", run.stdout);
    assert!(
        run.stderr.contains("external-command-not-found"),
        "{}",
        run.stderr
    );
}

#[test]
fn sync_files_refuses_a_project_that_was_never_added() {
    let home = temp_home();
    let base = home.path().join("Projects");
    std::fs::create_dir_all(&base).unwrap();
    write_config(home.path(), &base, "en");

    let run = sbxm(home.path(), &["--lang", "en", "sync-files", "owner/repo"]);
    assert_eq!(run.code, 1);
    assert!(run.stderr.contains("project-not-managed"), "{}", run.stderr);
    assert!(run.stderr.contains("sbxm add owner/repo"), "{}", run.stderr);
}

#[test]
fn option_mode_init_creates_a_private_configuration_without_a_terminal() {
    use std::os::unix::fs::PermissionsExt;
    let home = temp_home();
    let base = home.path().join("Projects");

    let run = sbxm(
        home.path(),
        &[
            "--lang",
            "en",
            "init",
            "--base-path",
            base.to_str().unwrap(),
            "--git-user-name",
            "Example User",
            "--git-user-email",
            "user@example.com",
        ],
    );

    assert_eq!(run.code, 0, "{}", run.stderr);
    assert!(run.stdout.contains("config.toml"), "{}", run.stdout);
    assert!(
        run.stdout.contains("sbxm status --global"),
        "{}",
        run.stdout
    );

    let config = home.path().join(".sbxm").join("config.toml");
    assert!(config.is_file());
    let mode = std::fs::metadata(&config).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
    assert!(base.is_dir(), "the base path is created");

    let written = std::fs::read_to_string(&config).unwrap();
    assert!(written.contains("version = 1"), "{written}");
    assert!(written.contains("language = \"en\""), "{written}");
    assert!(
        !written.contains("token") && !written.contains("secret"),
        "the configuration must not carry credentials: {written}"
    );
}

#[test]
fn re_running_init_changes_nothing_and_succeeds() {
    let home = temp_home();
    let base = home.path().join("Projects");
    let arguments = [
        "--lang",
        "ja",
        "init",
        "--base-path",
        base.to_str().unwrap(),
        "--git-user-name",
        "Example User",
        "--git-user-email",
        "user@example.com",
    ];

    let first = sbxm(home.path(), &arguments);
    assert_eq!(first.code, 0, "{}", first.stderr);
    let config = home.path().join(".sbxm").join("config.toml");
    let before = std::fs::read_to_string(&config).unwrap();

    let second = sbxm(home.path(), &arguments);
    assert_eq!(second.code, 0, "{}", second.stderr);
    assert_eq!(std::fs::read_to_string(&config).unwrap(), before);
    assert!(
        second.stdout.contains("初期化済み"),
        "the second run reports that nothing changed: {}",
        second.stdout
    );
}

#[test]
fn interactive_init_outside_a_terminal_creates_nothing() {
    let home = temp_home();
    let run = sbxm(home.path(), &["init"]);
    assert_eq!(run.code, 1);
    assert!(run.stderr.contains("init-requires-tty"), "{}", run.stderr);
    assert!(!home.path().join(".sbxm").join("config.toml").exists());
}

#[test]
fn global_status_prints_only_the_global_section_and_the_published_columns() {
    let home = temp_home();
    let run = sbxm(home.path(), &["--lang", "en", "status", "--global"]);

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
            "Config",
            "Base path",
            "Platform",
            "Git",
            "SSH",
            "Docker",
            "Docker Sandboxes",
            "Network policy",
            "Daemon",
            "Docker Sandboxes login",
            "Active session inspection",
        ]
    );
    assert!(!run.stdout.contains("PROJECT"), "{}", run.stdout);
    assert!(!run.stdout.contains("WORKTREES"), "{}", run.stdout);
}

#[test]
fn global_status_reports_every_problem_and_exits_with_one() {
    let home = temp_home();
    let run = sbxm(home.path(), &["--lang", "en", "status", "--global"]);

    assert_eq!(run.code, 1, "an incomplete host is not healthy");
    // 取得できた行は後続検査が失敗しても省略しない。
    assert_eq!(run.stdout.lines().count(), 13);
    for id in ["config-missing", "host-command-missing"] {
        assert!(
            run.stderr.contains(id),
            "{id} is missing from: {}",
            run.stderr
        );
    }
    // 詳細は表の列を増やさず、stderrの診断として出す。
    assert!(!run.stdout.contains("error:"), "{}", run.stdout);
}

#[test]
fn global_status_never_writes_to_the_host() {
    let home = temp_home();
    sbxm(home.path(), &["status", "--global"]);
    assert_eq!(
        std::fs::read_dir(home.path()).unwrap().count(),
        0,
        "a read-only diagnosis must not create anything under HOME"
    );
}

#[test]
fn the_japanese_mode_adds_a_legend_for_the_values_that_appeared() {
    let home = temp_home();
    let run = sbxm(home.path(), &["--lang", "ja", "status", "--global"]);

    assert!(run.stdout.contains("状態値の凡例"), "{}", run.stdout);
    assert!(run.stdout.contains("error:"), "{}", run.stdout);
    assert!(
        !run.stdout.contains("running:"),
        "values that did not appear must be left out: {}",
        run.stdout
    );

    // 英語modeには凡例を出さない。
    let english = sbxm(home.path(), &["--lang", "en", "status", "--global"]);
    assert!(!english.stdout.contains("Legend"), "{}", english.stdout);
}

#[test]
fn status_values_stay_untranslated_in_the_japanese_mode() {
    let home = temp_home();
    let run = sbxm(home.path(), &["--lang", "ja", "status", "--global"]);
    assert!(run.stdout.contains("項目 (ITEM)"), "{}", run.stdout);
    assert!(run.stdout.contains("設定 (Config)"), "{}", run.stdout);
    assert!(run.stdout.contains("missing"), "{}", run.stdout);
    assert!(run.stdout.contains("error"), "{}", run.stdout);
}
