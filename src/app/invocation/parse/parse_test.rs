use crate::commands::{self, Command};
use crate::design::ColorMode;
use crate::diagnostics::ErrorId;
use crate::i18n::Catalog;
use clap::{Arg, Command as ClapCommand};

use crate::testing::outcome::{Checked, Refused, Required, Unmet};

use super::{build_parser_for_test as build_command, parse};
use crate::app::invocation::Interactivity;
use crate::i18n::Locale;
use crate::testing::cli::{argv, command, non_tty, parse_argv, tty};

#[test]
fn the_lang_option_is_accepted_before_and_after_the_subcommand() -> Checked {
    assert!(matches!(
        command(&["--lang", "ja", "ls"], tty())?,
        Command::Ls
    ));
    assert!(matches!(
        command(&["ls", "--lang", "ja"], tty())?,
        Command::Ls
    ));
    Ok(())
}

#[test]
fn help_and_version_exit_successfully() -> Checked {
    let outcome = parse_argv(&["--help"], tty()).required_because("help is not a failure")?;
    let Command::Help(text) = outcome else {
        return Err(Unmet::new("--help must produce help text".to_string()));
    };
    assert!(text.contains("Usage:"), "{text}");
    assert!(text.contains("Commands:"), "{text}");

    let outcome = parse_argv(&["--version"], tty()).required_because("version is not a failure")?;
    assert_eq!(
        outcome,
        Command::Version(format!("sbxm {}", env!("CARGO_PKG_VERSION")))
    );
    Ok(())
}

#[test]
fn help_is_rendered_in_the_selected_language() -> Checked {
    let catalog = Catalog::new(Locale::Ja);
    let outcome = parse(&argv(&["--help"]), &catalog, tty()).required_because("help renders")?;
    let Command::Help(text) = outcome else {
        return Err(Unmet::new("--help must produce help text".to_string()));
    };
    assert!(text.contains("使い方 (Usage):"), "{text}");
    assert!(text.contains("command (Commands):"), "{text}");
    assert!(
        text.contains("案件ごとのDocker Sandbox"),
        "the about text must come from the Japanese resource: {text}"
    );
    Ok(())
}

#[test]
fn localized_help_lists_policy_values_without_clap_possible_values() -> Checked {
    for locale in Locale::ALL {
        let catalog = Catalog::new(locale);
        let outcome =
            parse(&argv(&["--help"]), &catalog, tty()).required_because("help renders")?;
        let Command::Help(text) = outcome else {
            return Err(Unmet::new(format!(
                "{locale}: --help must produce help text"
            )));
        };

        for (option, values) in [
            ("--lang", Locale::value_list()),
            ("--color", ColorMode::value_list()),
        ] {
            let line = text
                .lines()
                .find(|line| line.contains(option))
                .required_because(&format!("{locale}: {option} is shown in help"))?;
            assert!(
                line.contains(values.as_str()),
                "{locale}: {option} must list policy values in its localized help: {line}"
            );
        }
        assert!(
            !text.contains("[possible values:"),
            "{locale}: clap's fixed possible-values text must not be added to help: {text}"
        );
    }
    Ok(())
}

#[test]
fn rebuild_help_explains_that_recreation_loses_the_writable_layer() -> Checked {
    for (locale, expected) in [
        (
            Locale::En,
            [
                "whether or not it changed",
                "writable layer is lost",
                "protects work",
                "asks for confirmation",
            ],
        ),
        (
            Locale::Ja,
            [
                "変更有無にかかわらず",
                "書き込み可能な層は失われます",
                "作業を保護し",
                "確認を求めます",
            ],
        ),
    ] {
        let catalog = Catalog::new(locale);
        let outcome = parse(&argv(&["rebuild", "--help"]), &catalog, tty())
            .required_because("rebuild help renders")?;
        let Command::Help(text) = outcome else {
            return Err(Unmet::new(format!(
                "{locale}: rebuild help was not returned"
            )));
        };
        for fragment in expected {
            assert!(
                text.contains(fragment),
                "{locale}: rebuild help does not explain the writable layer: {text}"
            );
        }
    }
    Ok(())
}

/// 公開契約をlocaleに依存しない形で書き出す。
///
/// 翻訳文はlocaleごとに変わるため含めない。ここへ現れるのはcommand名、option名、
/// value name、arity、必須性、並び順といったCLIの契約だけとする。
fn render_surface() -> Checked<String> {
    let catalog = Catalog::new(Locale::SOURCE);
    let mut command = build_command(&catalog).required_because("the parser builds")?;
    // 上位から伝播するglobal optionを含めた実効の姿を記録する。
    command.build();
    let mut out = String::new();
    render_command(&command, 0, &mut out);
    Ok(out)
}

fn render_command(command: &ClapCommand, depth: usize, out: &mut String) {
    use std::fmt::Write as _;

    let indent = "  ".repeat(depth);
    let _ = writeln!(out, "{indent}{}", command.get_name());
    for argument in command.get_arguments() {
        let _ = writeln!(out, "{indent}  {}", render_argument(argument));
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
        let names: Vec<&str> = names.iter().map(clap::builder::Str::as_str).collect();
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
fn the_published_contract_matches_the_recorded_surface() -> Checked {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("snapshots")
        .join("cli-surface.txt");
    let actual = render_surface()?;

    if std::env::var_os("SBXM_UPDATE_SNAPSHOTS").is_some() {
        std::fs::create_dir_all(
            path.parent()
                .required_because("the snapshot has a directory")?,
        )
        .required_because("create the snapshot directory")?;
        std::fs::write(&path, &actual).required_because("write the contract record")?;
        return Ok(());
    }

    let expected = std::fs::read_to_string(&path).required_because(&format!(
        "{} could not be read. Run with SBXM_UPDATE_SNAPSHOTS=1 to create it.",
        path.display()
    ))?;
    assert_eq!(
        actual,
        expected,
        "the published CLI contract changed. Review {} before accepting it, then run with SBXM_UPDATE_SNAPSHOTS=1.",
        path.display()
    );
    Ok(())
}

/// 利用者へ見える全slotが、選んだlocaleのresourceで埋まっている。
///
/// `.help()`と`.about()`の付け忘れを、helpを描画せずに検出する。message IDの欠落は
/// parserの構築自体が失敗するため、ここで併せて落ちる。
#[test]
fn every_visible_slot_is_filled_from_the_resource() -> Checked {
    for locale in Locale::ALL {
        let catalog = Catalog::new(locale);
        let mut command = build_command(&catalog).required_because("the parser builds")?;
        command.build();

        let mut slots = Vec::new();
        collect_strings(&command, "sbxm", &mut slots);
        assert!(!slots.is_empty(), "{locale}: nothing was collected");

        for (slot, text) in slots {
            let text = text.required_because(&format!(
                "{locale}: {slot} has no text; every slot comes from the resource"
            ))?;
            assert!(!text.trim().is_empty(), "{locale}: {slot} is empty");
        }
    }
    Ok(())
}

/// 利用者へ見える文字列slotを、対象と現在値の組で集める。
fn collect_strings(command: &ClapCommand, path: &str, out: &mut Vec<(String, Option<String>)>) {
    out.push((
        format!("{path} (about)"),
        command.get_about().map(std::string::ToString::to_string),
    ));
    for argument in command.get_arguments() {
        out.push((
            format!("{path} {} (help)", argument.get_id()),
            argument.get_help().map(std::string::ToString::to_string),
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
fn each_subcommand_renders_its_own_help() -> Checked {
    for name in [
        "add", "prepare", "apply", "rebuild", "open", "stop", "ls", "status", "destroy",
    ] {
        let outcome =
            parse_argv(&[name, "--help"], tty()).required_because("subcommand help renders")?;
        let Command::Help(text) = outcome else {
            return Err(Unmet::new(format!("{name} --help must produce help text")));
        };
        assert!(text.contains("Usage:"), "{name}: {text}");
        assert!(text.contains("--lang"), "{name}: {text}");
    }
    Ok(())
}

#[test]
fn a_command_that_always_needs_a_project_refuses_to_prompt() -> Checked {
    // `add`は未登録のprojectを対象とするため、選ぶ候補が存在しない。
    let error = parse_argv(&["add"], tty()).refused_because("add requires a project")?;
    assert_eq!(error.first_id(), Some(ErrorId::MissingRequiredArgument));
    Ok(())
}

#[test]
fn omitting_the_target_outside_a_terminal_is_a_usage_error() -> Checked {
    for arguments in [
        vec!["prepare"],
        vec!["apply", "--files"],
        vec!["rebuild"],
        vec!["open"],
        vec!["stop"],
        vec!["destroy"],
    ] {
        let error = parse_argv(&arguments, non_tty())
            .refused_because("a non-interactive run needs an explicit target")?;
        assert_eq!(
            error.first_id(),
            Some(ErrorId::ProjectArgumentRequired),
            "{arguments:?} produced the wrong error"
        );
    }
    Ok(())
}

#[test]
fn omitting_the_target_on_a_terminal_defers_to_the_selection_prompt() -> Checked {
    assert_eq!(command(&["prepare"], tty())?, Command::Prepare(None));
    assert_eq!(
        command(&["apply", "--files"], tty())?,
        Command::Apply(commands::apply::Args {
            project: None,
            files: true,
            worktrees: None,
        })
    );
    assert_eq!(command(&["rebuild"], tty())?, Command::Rebuild(None));
    assert_eq!(
        command(&["open"], tty())?,
        Command::Open(commands::open::Args {
            project: None,
            index: None,
        })
    );
    assert_eq!(command(&["stop"], tty())?, Command::Stop(Vec::new()));
    assert_eq!(
        command(&["destroy"], tty())?,
        Command::Destroy(commands::destroy::Args {
            project: None,
            force: false
        })
    );
    Ok(())
}

#[test]
fn open_accepts_a_zero_based_worktree_index() -> Checked {
    let project = crate::project::ProjectId::parse("crescware/sbxm")?;
    assert_eq!(
        command(&["open", "crescware/sbxm", "-i", "0"], tty())?,
        Command::Open(commands::open::Args {
            project: Some(project),
            index: Some(0),
        })
    );
    Ok(())
}

#[test]
fn a_prompt_needs_both_stdin_and_stderr_to_be_a_terminal() -> Checked {
    for (stdin_is_tty, stderr_is_tty) in [(true, false), (false, true)] {
        let interactivity = Interactivity::from_streams(stdin_is_tty, stderr_is_tty);
        let error = parse_argv(&["open"], interactivity)
            .refused_because("both streams must be a terminal to prompt")?;
        assert_eq!(error.first_id(), Some(ErrorId::ProjectArgumentRequired));
    }
    Ok(())
}

#[test]
fn an_invalid_project_identifier_is_refused_by_every_command_that_takes_one() -> Checked {
    for arguments in [
        vec!["prepare", "owner/repo/extra"],
        vec!["apply", "--files", "owner/repo/extra"],
        vec!["rebuild", "/repo"],
        vec!["open", "owner/"],
        vec!["stop", "owner"],
        vec!["status", "owner//repo"],
        vec!["destroy", "owner/repo/x"],
    ] {
        let error =
            parse_argv(&arguments, tty()).refused_because("{arguments:?} must be refused")?;
        assert_eq!(
            error.first_id(),
            Some(ErrorId::InvalidProjectId),
            "{arguments:?} produced the wrong error"
        );
    }
    Ok(())
}

#[test]
fn unknown_arguments_and_commands_are_named_in_the_diagnostic() -> Checked {
    let error =
        parse_argv(&["ls", "--nope"], tty()).refused_because("unknown options are refused")?;
    assert_eq!(error.first_id(), Some(ErrorId::UnknownArgument));

    let error = parse_argv(&["nope"], tty()).refused_because("unknown commands are refused")?;
    assert_eq!(error.first_id(), Some(ErrorId::UnknownSubcommand));
    Ok(())
}

#[test]
fn a_missing_command_is_a_usage_error_rather_than_a_default_action() -> Checked {
    let error = parse_argv(&[], tty()).refused_because("sbxm alone does not act")?;
    assert_eq!(error.first_id(), Some(ErrorId::MissingSubcommand));
    Ok(())
}

#[test]
fn an_invalid_lang_value_is_reported_as_a_value_error_by_the_parser() -> Checked {
    // 組み込みlocaleにならないtagを使う。
    let error = parse_argv(&["--lang", "zz", "ls"], tty()).refused_because("zz is not a locale")?;
    assert_eq!(error.first_id(), Some(ErrorId::InvalidValue));
    Ok(())
}
