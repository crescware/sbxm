use super::*;

fn argv(arguments: &[&str]) -> Vec<String> {
    std::iter::once("sbxm".to_string())
        .chain(arguments.iter().map(|value| value.to_string()))
        .collect()
}

fn tty() -> Interactivity {
    Interactivity {
        stdin_is_tty: true,
        stderr_is_tty: true,
    }
}

fn non_tty() -> Interactivity {
    Interactivity {
        stdin_is_tty: false,
        stderr_is_tty: false,
    }
}

fn run(arguments: &[&str], interactivity: Interactivity) -> Result<Outcome> {
    let catalog = Catalog::new(Locale::En);
    parse(&argv(arguments), &catalog, interactivity)
}

fn command(arguments: &[&str], interactivity: Interactivity) -> Command {
    match run(arguments, interactivity).expect("the arguments parse") {
        Outcome::Run(command) => command,
        other => panic!("expected a command, got {other:?}"),
    }
}

#[test]
fn lang_is_read_before_the_parser_runs_and_from_either_side() {
    assert_eq!(
        peek_lang(&argv(&["--lang", "ja", "init"])),
        PeekedLang::Valid(Locale::Ja)
    );
    assert_eq!(
        peek_lang(&argv(&["init", "--lang", "ja"])),
        PeekedLang::Valid(Locale::Ja)
    );
    assert_eq!(
        peek_lang(&argv(&["--lang=en", "ls"])),
        PeekedLang::Valid(Locale::En)
    );
    assert_eq!(peek_lang(&argv(&["ls"])), PeekedLang::Absent);
    assert_eq!(
        peek_lang(&argv(&["--lang", "zz", "ls"])),
        PeekedLang::Invalid("zz".to_string())
    );
    // `--`以降は先読みしない。
    assert_eq!(
        peek_lang(&argv(&["ls", "--", "--lang", "ja"])),
        PeekedLang::Absent
    );
}

#[test]
fn the_lang_option_is_accepted_before_and_after_the_subcommand() {
    assert!(matches!(
        command(&["--lang", "ja", "ls"], tty()),
        Command::Ls
    ));
    assert!(matches!(
        command(&["ls", "--lang", "ja"], tty()),
        Command::Ls
    ));
}

#[test]
fn an_unsupported_language_is_rejected_without_reading_anything_else() {
    let error = invalid_lang_error("zz");
    assert_eq!(error.first_id(), Some(ErrorId::InvalidLang));
}

#[test]
fn help_and_version_exit_successfully() {
    let outcome = run(&["--help"], tty()).expect("help is not a failure");
    let Outcome::Help(text) = outcome else {
        panic!("--help must produce help text");
    };
    assert!(text.contains("Usage:"), "{text}");
    assert!(text.contains("Commands:"), "{text}");

    let outcome = run(&["--version"], tty()).expect("version is not a failure");
    assert_eq!(
        outcome,
        Outcome::Version(format!("sbxm {}", env!("CARGO_PKG_VERSION")))
    );
}

#[test]
fn help_is_rendered_in_the_selected_language() {
    let catalog = Catalog::new(Locale::Ja);
    let outcome = parse(&argv(&["--help"]), &catalog, tty()).expect("help renders");
    let Outcome::Help(text) = outcome else {
        panic!("--help must produce help text");
    };
    assert!(text.contains("使い方 (Usage):"), "{text}");
    assert!(text.contains("command (Commands):"), "{text}");
    assert!(
        text.contains("案件ごとのDocker Sandbox"),
        "the about text must come from the Japanese resource: {text}"
    );
}

/// 公開契約をlocaleに依存しない形で書き出す。
///
/// 翻訳文はlocaleごとに変わるため含めない。ここへ現れるのはcommand名、option名、
/// value name、arity、必須性、並び順といったCLIの契約だけとする。
fn render_surface() -> String {
    let catalog = Catalog::new(Locale::SOURCE);
    let mut command = build_command(&catalog).expect("the parser builds");
    // 上位から伝播するglobal optionを含めた実効の姿を記録する。
    command.build();
    let mut out = String::new();
    render_command(&command, 0, &mut out);
    out
}

fn render_command(command: &ClapCommand, depth: usize, out: &mut String) {
    let indent = "  ".repeat(depth);
    out.push_str(&format!("{indent}{}\n", command.get_name()));
    for argument in command.get_arguments() {
        out.push_str(&format!("{indent}  {}\n", render_argument(argument)));
    }
    for subcommand in command.get_subcommands() {
        render_command(subcommand, depth + 1, out);
    }
}

fn render_argument(argument: &Arg) -> String {
    let mut parts = vec![format!("id={}", argument.get_id())];
    if argument.is_positional() {
        parts.push("positional".to_string());
    }
    if let Some(long) = argument.get_long() {
        parts.push(format!("--{long}"));
    }
    if let Some(short) = argument.get_short() {
        parts.push(format!("-{short}"));
    }
    // `--lang`が受け付ける値とその表示は組み込みlocaleの定義から導出される。
    // 契約記録へ言語の数を持ち込まないため、導出であることだけを書く。
    if argument.get_id() == "lang" {
        parts.push("value=<derived from the locale definitions>".to_string());
    } else if let Some(names) = argument.get_value_names() {
        let names: Vec<&str> = names.iter().map(|name| name.as_str()).collect();
        parts.push(format!("value={}", names.join(",")));
    }
    if let Some(range) = argument.get_num_args() {
        parts.push(format!("args={range}"));
    }
    if argument.is_required_set() {
        parts.push("required".to_string());
    }
    if argument.is_global_set() {
        parts.push("global".to_string());
    }
    parts.push(format!("action={:?}", argument.get_action()));
    parts.push(format!("order={}", argument.get_display_order()));
    parts.join(" ")
}

/// CLIの公開契約。localeに依存しないため、言語を増やしても変わらない。
///
/// 実装から導出した期待値と突き合わせても契約の変化は捕まらないため、記録をcommitし、
/// 契約を変えるときはこのfileの差分をreviewさせる。`SBXM_UPDATE_SNAPSHOTS=1`で更新する。
#[test]
fn the_published_contract_matches_the_recorded_surface() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("snapshots")
        .join("cli-surface.txt");
    let actual = render_surface();

    if std::env::var_os("SBXM_UPDATE_SNAPSHOTS").is_some() {
        std::fs::create_dir_all(path.parent().expect("the snapshot has a directory"))
            .expect("create the snapshot directory");
        std::fs::write(&path, &actual).expect("write the contract record");
        return;
    }

    let expected = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "{} could not be read: {error}. Run with SBXM_UPDATE_SNAPSHOTS=1 to create it.",
            path.display()
        )
    });
    assert_eq!(
        actual,
        expected,
        "the published CLI contract changed. Review {} before accepting it, then run with SBXM_UPDATE_SNAPSHOTS=1.",
        path.display()
    );
}

/// 利用者へ見える全slotが、選んだlocaleのresourceで埋まっている。
///
/// `.help()`と`.about()`の付け忘れを、helpを描画せずに検出する。message IDの欠落は
/// parserの構築自体が失敗するため、ここで併せて落ちる。
#[test]
fn every_visible_slot_is_filled_from_the_resource() {
    for locale in Locale::ALL {
        let catalog = Catalog::new(locale);
        let mut command = build_command(&catalog).expect("the parser builds");
        command.build();

        let mut slots = Vec::new();
        collect_strings(&command, "sbxm", &mut slots);
        assert!(!slots.is_empty(), "{locale}: nothing was collected");

        for (slot, text) in slots {
            let text = text.unwrap_or_else(|| {
                panic!("{locale}: {slot} has no text; every slot comes from the resource")
            });
            assert!(!text.trim().is_empty(), "{locale}: {slot} is empty");
        }
    }
}

/// 利用者へ見える文字列slotを、対象と現在値の組で集める。
fn collect_strings(command: &ClapCommand, path: &str, out: &mut Vec<(String, Option<String>)>) {
    out.push((
        format!("{path} (about)"),
        command.get_about().map(|about| about.to_string()),
    ));
    for argument in command.get_arguments() {
        out.push((
            format!("{path} {} (help)", argument.get_id()),
            argument.get_help().map(|help| help.to_string()),
        ));
    }
    for subcommand in command.get_subcommands() {
        collect_strings(
            subcommand,
            &format!("{path} {}", subcommand.get_name()),
            out,
        );
    }
}

#[test]
fn each_subcommand_renders_its_own_help() {
    for name in [
        "init", "add", "apply", "rebuild", "open", "stop", "ls", "status", "destroy",
    ] {
        let outcome = run(&[name, "--help"], tty()).expect("subcommand help renders");
        let Outcome::Help(text) = outcome else {
            panic!("{name} --help must produce help text");
        };
        assert!(text.contains("Usage:"), "{name}: {text}");
        assert!(text.contains("--lang"), "{name}: {text}");
    }
}

#[test]
fn init_accepts_either_no_options_or_all_three() {
    assert_eq!(
        command(&["init"], non_tty()),
        Command::Init(InitMode::Interactive)
    );
    assert_eq!(
        command(
            &[
                "init",
                "--base-path",
                "/Users/example/Projects",
                "--git-user-name",
                "Example User",
                "--git-user-email",
                "user@example.com"
            ],
            non_tty()
        ),
        Command::Init(InitMode::Options {
            base_path: "/Users/example/Projects".into(),
            git_user_name: "Example User".into(),
            git_user_email: "user@example.com".into(),
        })
    );
}

#[test]
fn a_partially_specified_init_is_refused_before_anything_is_read() {
    let error = run(&["init", "--base-path", "/tmp/projects"], tty())
        .expect_err("a partial option set is refused");
    assert_eq!(error.first_id(), Some(ErrorId::InitIncompleteOptions));
    let rendered = Catalog::new(Locale::En)
        .format(&error.diagnostics()[0].description)
        .expect("the diagnostic formats");
    assert!(rendered.contains("--git-user-name"), "{rendered}");
    assert!(rendered.contains("--git-user-email"), "{rendered}");
}

#[test]
fn the_init_mode_is_decided_without_looking_at_lang() {
    assert_eq!(
        command(&["--lang", "ja", "init"], non_tty()),
        Command::Init(InitMode::Interactive)
    );
    assert_eq!(
        command(
            &[
                "init",
                "--lang",
                "en",
                "--base-path",
                "/tmp/p",
                "--git-user-name",
                "n",
                "--git-user-email",
                "e"
            ],
            non_tty()
        ),
        Command::Init(InitMode::Options {
            base_path: "/tmp/p".into(),
            git_user_name: "n".into(),
            git_user_email: "e".into(),
        })
    );
}

#[test]
fn worktree_counts_outside_the_allowed_range_are_refused() {
    for value in ["0", "33", "999", "abc", ""] {
        let error = run(&["add", "owner/repo", "--worktrees", value], tty())
            .expect_err("{value} must be refused");
        assert_eq!(
            error.first_id(),
            Some(ErrorId::WorktreesOutOfRange),
            "value {value} produced the wrong error"
        );
    }
    // 負値はoptionとして解釈されるため、値の範囲ではなくsyntaxの段階で止まる。
    let error = run(&["add", "owner/repo", "--worktrees", "-1"], tty())
        .expect_err("a negative count never reaches the range check");
    assert_eq!(error.exit_code(), crate::error::ExitCode::Failure);
    assert!(matches!(
        command(&["add", "owner/repo", "--worktrees", "1"], tty()),
        Command::Add(AddArgs {
            worktrees: Some(1),
            ..
        })
    ));
    assert!(matches!(
        command(
            &[
                "add",
                "owner/repo",
                "--worktrees",
                "32",
                "--detach",
                "develop"
            ],
            tty()
        ),
        Command::Add(AddArgs {
            worktrees: Some(32),
            ..
        })
    ));
}

#[test]
fn more_than_one_worktree_requires_an_explicit_start_branch() {
    let error = run(&["add", "owner/repo", "--worktrees", "2"], tty())
        .expect_err("two worktrees without a branch are refused");
    assert_eq!(error.first_id(), Some(ErrorId::WorktreesRequireDetach));

    assert!(matches!(
        command(
            &[
                "add",
                "owner/repo",
                "--worktrees",
                "2",
                "--detach",
                "develop"
            ],
            tty()
        ),
        Command::Add(_)
    ));
}

#[test]
fn apply_requires_an_explicit_scope() {
    let error = run(&["apply", "owner/repo"], tty()).expect_err("apply without a scope is refused");
    assert_eq!(error.first_id(), Some(ErrorId::ApplyScopeRequired));

    assert!(matches!(
        command(&["apply", "owner/repo", "--files"], tty()),
        Command::Apply(ApplyArgs {
            files: true,
            worktrees: None,
            ..
        })
    ));
    assert!(matches!(
        command(&["apply", "owner/repo", "--worktrees", "3"], tty()),
        Command::Apply(ApplyArgs {
            files: false,
            worktrees: Some(3),
            ..
        })
    ));
    assert!(matches!(
        command(
            &["apply", "owner/repo", "--files", "--worktrees", "3"],
            tty()
        ),
        Command::Apply(ApplyArgs {
            files: true,
            worktrees: Some(3),
            ..
        })
    ));
}

#[test]
fn commands_that_always_need_a_project_refuse_to_prompt() {
    for name in ["add", "apply", "rebuild"] {
        let error = run(&[name], tty()).expect_err("{name} requires a project");
        assert_eq!(
            error.first_id(),
            Some(ErrorId::MissingRequiredArgument),
            "{name} produced the wrong error"
        );
    }
}

#[test]
fn status_requires_exactly_one_scope() {
    assert_eq!(
        command(&["status", "--global"], tty()),
        Command::Status(StatusScope::Global)
    );
    assert_eq!(
        command(&["status", "-g"], tty()),
        Command::Status(StatusScope::Global)
    );
    assert!(matches!(
        command(&["status", "owner/repo"], tty()),
        Command::Status(StatusScope::Project(_))
    ));

    for arguments in [vec!["status"], vec!["status", "--global", "owner/repo"]] {
        let error = run(&arguments, tty()).expect_err("exactly one scope is required");
        assert_eq!(error.first_id(), Some(ErrorId::StatusScopeRequired));
    }
}

#[test]
fn status_never_prompts_even_on_a_terminal() {
    let error = run(&["status"], tty()).expect_err("status does not offer a project prompt");
    assert_eq!(error.first_id(), Some(ErrorId::StatusScopeRequired));
}

#[test]
fn omitting_the_target_outside_a_terminal_is_a_usage_error() {
    for arguments in [vec!["open"], vec!["stop"], vec!["destroy"]] {
        let error =
            run(&arguments, non_tty()).expect_err("a non-interactive run needs an explicit target");
        assert_eq!(
            error.first_id(),
            Some(ErrorId::ProjectArgumentRequired),
            "{arguments:?} produced the wrong error"
        );
    }
}

#[test]
fn omitting_the_target_on_a_terminal_defers_to_the_selection_prompt() {
    assert_eq!(command(&["open"], tty()), Command::Open(None));
    assert_eq!(command(&["stop"], tty()), Command::Stop(Vec::new()));
    assert_eq!(
        command(&["destroy"], tty()),
        Command::Destroy(DestroyArgs {
            project: None,
            force: false
        })
    );
}

#[test]
fn a_prompt_needs_both_stdin_and_stderr_to_be_a_terminal() {
    for interactivity in [
        Interactivity {
            stdin_is_tty: true,
            stderr_is_tty: false,
        },
        Interactivity {
            stdin_is_tty: false,
            stderr_is_tty: true,
        },
    ] {
        let error =
            run(&["open"], interactivity).expect_err("both streams must be a terminal to prompt");
        assert_eq!(error.first_id(), Some(ErrorId::ProjectArgumentRequired));
    }
}

#[test]
fn forced_destroy_always_requires_a_fully_specified_project() {
    for flag in ["--force", "-f"] {
        let error =
            run(&["destroy", flag], tty()).expect_err("force mode never prompts for a target");
        assert_eq!(error.first_id(), Some(ErrorId::ProjectArgumentRequired));

        assert_eq!(
            command(&["destroy", flag, "owner/repo"], non_tty()),
            Command::Destroy(DestroyArgs {
                project: Some(ProjectId::parse("owner/repo").unwrap()),
                force: true
            })
        );
    }
}

#[test]
fn stop_accepts_several_projects() {
    assert_eq!(
        command(&["stop", "owner/one", "owner/two"], non_tty()),
        Command::Stop(vec![
            ProjectId::parse("owner/one").unwrap(),
            ProjectId::parse("owner/two").unwrap(),
        ])
    );
}

#[test]
fn an_invalid_project_identifier_is_refused_by_every_command_that_takes_one() {
    for arguments in [
        vec!["add", "not-a-project"],
        vec!["apply", "--files", "owner/repo/extra"],
        vec!["rebuild", "/repo"],
        vec!["open", "owner/"],
        vec!["stop", "owner"],
        vec!["status", "owner//repo"],
        vec!["destroy", "owner/repo/x"],
    ] {
        let error = run(&arguments, tty()).expect_err("{arguments:?} must be refused");
        assert_eq!(
            error.first_id(),
            Some(ErrorId::InvalidProjectId),
            "{arguments:?} produced the wrong error"
        );
    }
}

#[test]
fn unknown_arguments_and_commands_are_named_in_the_diagnostic() {
    let error = run(&["ls", "--nope"], tty()).expect_err("unknown options are refused");
    assert_eq!(error.first_id(), Some(ErrorId::UnknownArgument));

    let error = run(&["nope"], tty()).expect_err("unknown commands are refused");
    assert_eq!(error.first_id(), Some(ErrorId::UnknownSubcommand));
}

#[test]
fn a_missing_command_is_a_usage_error_rather_than_a_default_action() {
    let error = run(&[], tty()).expect_err("sbxm alone does not act");
    assert_eq!(error.first_id(), Some(ErrorId::MissingSubcommand));
}

#[test]
fn an_invalid_lang_value_is_reported_as_a_value_error_by_the_parser() {
    // 組み込みlocaleにならないtagを使う。
    let error = run(&["--lang", "zz", "ls"], tty()).expect_err("zz is not a locale");
    assert_eq!(error.first_id(), Some(ErrorId::InvalidValue));
}
