//! CLIの公開契約。
//!
//! exit codeは`0`、`1`、`130`だけを使う。CLI parserを含む内部libraryの既定exit codeを
//! 公開契約へ透過しない。helpとusageは選択したlocaleで生成する。

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
    sbxm_in(home, home, arguments)
}

/// 親directoryを明示してsbxmを実行する。
///
/// `add`は実行時のcurrent directoryへproject rootを足す。どこで実行したかがそのまま
/// 配置先になるため、testもcwdを明示する。
fn sbxm_in(home: &Path, cwd: &Path, arguments: &[&str]) -> Run {
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
        .expect("sbxm runs");
    Run::from(output)
}

fn temp_home() -> tempfile::TempDir {
    tempfile::tempdir().expect("temporary home")
}

/// hostのGit identityだけに答える`git`を用意し、そのdirectoryを返す。
///
/// `add`は案件を作る前にhostのGit identityを読む。PATHを空にしたままでは、その手前で
/// 止まって登録の契約を確かめられない。cloneには答えないため、実行はcloneで止まる。
fn fake_git(home: &Path) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let bin = home.join("bin");
    std::fs::create_dir_all(&bin).expect("create the fake bin directory");
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
    .expect("write the fake git");
    std::fs::set_permissions(&git, std::fs::Permissions::from_mode(0o755)).expect("mode");
    bin
}

/// hostのGit identityだけに答える`git`を置いたPATHでsbxmを実行する。
fn sbxm_with_git(home: &Path, cwd: &Path, arguments: &[&str]) -> Run {
    let bin = fake_git(home);
    let output = Command::new(env!("CARGO_BIN_EXE_sbxm"))
        .args(arguments)
        .current_dir(cwd)
        .env("HOME", home)
        .env("LC_ALL", "C")
        .env_remove("LC_MESSAGES")
        .env_remove("LANG")
        .env("PATH", &bin)
        .output()
        .expect("sbxm runs");
    Run::from(output)
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

fn write_config(home: &Path, language: &str) {
    use std::os::unix::fs::PermissionsExt;
    let dir = home.join(".sbxm");
    std::fs::create_dir_all(&dir).expect("create ~/.sbxm");
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).expect("mode");
    let path = dir.join("config.yaml");
    std::fs::write(&path, format!("version: 1\nlanguage: {language}\n")).expect("write config");
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
    write_config(home.path(), "ja");

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
    std::fs::write(dir.join("config.yaml"), "version: 99\n").unwrap();
    std::fs::set_permissions(
        dir.join("config.yaml"),
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
fn an_unsupported_language_is_reported_before_any_configuration_error() {
    use std::os::unix::fs::PermissionsExt;

    let home = temp_home();
    // 実在しないtagを使う。将来どの言語を足してもこのtestは意味を保つ。
    let run = sbxm(home.path(), &["--lang", "zz", "ls"]);
    assert_eq!(run.code, 1);
    assert!(run.stderr.contains("invalid-lang"), "{}", run.stderr);
    assert!(run.stdout.is_empty(), "{}", run.stdout);

    // 壊れたconfigはparse errorを覆い隠さない。
    let dir = home.path().join(".sbxm");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
    std::fs::write(dir.join("config.yaml"), "version: 99\n").unwrap();
    std::fs::set_permissions(
        dir.join("config.yaml"),
        std::fs::Permissions::from_mode(0o600),
    )
    .unwrap();

    let run = sbxm(home.path(), &["--lang", "zz", "ls"]);
    assert_eq!(run.code, 1);
    assert!(run.stderr.contains("invalid-lang"), "{}", run.stderr);
    assert!(
        !run.stderr.contains("config-unknown-version"),
        "the broken config must not be diagnosed instead: {}",
        run.stderr
    );

    // 読めるconfigがあっても、`--lang`の不正はそのまま失敗する。診断の本文は
    // configが宣言したlocaleで出す。resourceを複製せず、2回の実行の差で見る。
    write_config(home.path(), "en");
    let english = sbxm(home.path(), &["--lang", "zz", "ls"]);
    assert_eq!(english.code, 1);
    assert!(
        english.stderr.contains("invalid-lang"),
        "{}",
        english.stderr
    );

    write_config(home.path(), "ja");
    let japanese = sbxm(home.path(), &["--lang", "zz", "ls"]);
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
}

#[test]
fn parser_failures_map_to_exit_code_one() {
    let home = temp_home();
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
        let run = sbxm(home.path(), &arguments);
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
}

#[test]
fn diagnostics_name_a_stable_error_id() {
    let home = temp_home();
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
fn a_missing_configuration_is_the_defaults_rather_than_a_reason_to_stop() {
    let home = temp_home();
    // configが無くても、commandはhost toolまで進む。
    let run = sbxm(home.path(), &["ls"]);
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
}

#[test]
fn add_registers_the_project_before_it_reaches_the_host_tools() {
    use std::os::unix::fs::PermissionsExt;
    let home = temp_home();
    let base = home.path().join("Projects");
    std::fs::create_dir_all(&base).unwrap();
    write_config(home.path(), "en");

    let run = sbxm_with_git(
        home.path(),
        &base,
        &[
            "--lang",
            "en",
            "add",
            "git@github.com:Example-Org/Example-Repo.git",
        ],
    );
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
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );

    // 索引は`~/.sbxm/registry.yaml`が持ち、project rootを絶対pathで指す。
    let registry = home.path().join(".sbxm").join("registry.yaml");
    let index = std::fs::read_to_string(&registry).unwrap();
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
        std::fs::metadata(&registry).unwrap().permissions().mode() & 0o777,
        0o600
    );

    let written = std::fs::read_to_string(&metadata).unwrap();
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
}

#[test]
fn ls_needs_the_sandbox_runtime_before_it_can_answer() {
    let home = temp_home();
    let base = home.path().join("Projects");
    std::fs::create_dir_all(&base).unwrap();
    write_config(home.path(), "en");

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

/// 案件を登録した状態のHOMEを作る。
///
/// `add`はhost toolに到達した時点で止まるが、登録そのものは終わっている。
fn home_with_project(project: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    let home = temp_home();
    let base = home.path().join("Projects");
    std::fs::create_dir_all(&base).unwrap();
    write_config(home.path(), "en");
    let url = format!("git@github.com:{project}.git");
    let run = sbxm_with_git(home.path(), &base, &["--lang", "en", "add", &url]);
    assert_eq!(run.code, 1, "{}", run.stderr);
    (home, base)
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
fn project_status_keeps_the_items_it_could_read_and_names_the_global_command() {
    let (home, _base) = home_with_project("owner/repo");

    // host toolが無い環境でも、取得できた項目は後続検査の失敗にかかわらず表示する。
    let run = sbxm(home.path(), &["--lang", "en", "status", "owner/repo"]);
    assert_eq!(run.code, 1, "{}{}", run.stdout, run.stderr);

    let (project, worktrees) = run
        .stdout
        .split_once("\nWORKTREES\n")
        .expect("both sections are shown, even with nothing to list");
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
}

#[test]
fn the_japanese_project_status_translates_the_labels_and_keeps_the_values() {
    let (home, _base) = home_with_project("owner/repo");

    let run = sbxm(home.path(), &["--lang", "ja", "status", "owner/repo"]);
    assert_eq!(run.code, 1, "{}{}", run.stdout, run.stderr);
    // section名と項目名は訳し、状態値は訳さない。
    assert!(run.stdout.contains("案件 (PROJECT)"), "{}", run.stdout);
    assert!(run.stdout.contains("mismatch"), "{}", run.stdout);
    // 凡例はSandboxの状態を説明し、host serviceの説明を流用しない。
    assert!(!run.stdout.contains("service"), "{}", run.stdout);
}

#[test]
fn apply_refuses_a_project_that_was_never_added() {
    let home = temp_home();
    let base = home.path().join("Projects");
    std::fs::create_dir_all(&base).unwrap();
    write_config(home.path(), "en");

    let run = sbxm(
        home.path(),
        &["--lang", "en", "apply", "--files", "owner/repo"],
    );
    assert_eq!(run.code, 1);
    assert!(run.stderr.contains("project-not-managed"), "{}", run.stderr);
    assert!(
        run.stderr.contains("sbxm add <github-clone-url>"),
        "{}",
        run.stderr
    );
}

#[test]
fn a_run_that_cannot_prompt_neither_asks_for_a_language_nor_saves_one() {
    let home = temp_home();
    let base = home.path().join("Projects");
    std::fs::create_dir_all(&base).unwrap();

    // 非対話の`add`はpromptを出さず、言語を永続化しない。`--lang`も保存しない。
    let run = sbxm_with_git(
        home.path(),
        &base,
        &[
            "--lang",
            "ja",
            "add",
            "git@github.com:Example-Org/Example-Repo.git",
        ],
    );
    assert_eq!(run.code, 1, "{}", run.stderr);
    assert!(
        !home.path().join(".sbxm").join("config.yaml").exists(),
        "the display language is the user's to choose, not a side effect of --lang"
    );
    // それでも登録は進む。registryは作られる。
    assert!(home.path().join(".sbxm").join("registry.yaml").is_file());
}

#[test]
fn read_only_commands_never_create_a_configuration() {
    let home = temp_home();
    for arguments in [vec!["ls"], vec!["status", "--global"], vec!["--help"]] {
        let run = sbxm(home.path(), &arguments);
        assert!(
            !home.path().join(".sbxm").join("config.yaml").exists(),
            "{arguments:?} created a configuration: {}",
            run.stderr
        );
    }
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
}

#[test]
fn global_status_reports_every_problem_and_exits_with_one() {
    let home = temp_home();
    let run = sbxm(home.path(), &["--lang", "en", "status", "--global"]);

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

    let legend = run
        .stdout
        .split_once("状態値の凡例\n")
        .expect("the legend is its own section")
        .1;
    // 値は訳さず、説明だけを訳す。
    assert!(legend.contains("error "), "{legend}");
    assert!(
        !legend.contains("running"),
        "values that did not appear must be left out: {legend}"
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
