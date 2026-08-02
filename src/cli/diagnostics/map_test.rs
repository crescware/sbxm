use clap::{Arg, ArgAction, Command as ClapCommand};

use crate::cli::diagnostics::interpret;
use crate::diagnostics::{Diagnostic, ErrorId};
use crate::testing::cli::{parse_argv, tty};
use crate::testing::outcome::{Checked, Refused, Required};

/// 引数どうしの衝突は、sbxmの文法では起こせない。
///
/// 衝突を宣言したparserを別に組み、libraryが返すerrorの形だけを本物にする。写像が
/// 見るのはerrorの種類とcontextであり、どのcommandが返したかではない。
fn conflicting_parser() -> ClapCommand {
    ClapCommand::new("probe")
        .arg(Arg::new("one").long("one").action(ArgAction::SetTrue))
        .arg(
            Arg::new("two")
                .long("two")
                .action(ArgAction::SetTrue)
                .conflicts_with("one"),
        )
        // 単独でしか使えない引数は、衝突相手を伴わないerrorを返す。
        .arg(
            Arg::new("alone")
                .long("alone")
                .action(ArgAction::SetTrue)
                .exclusive(true),
        )
        // subcommandとの衝突は、後から来た側を引数として名指さない。
        .args_conflicts_with_subcommands(true)
        .subcommand(ClapCommand::new("sub"))
}

/// CLIと同じ入口で、libraryのerrorを診断へ写像する。
fn diagnostic_for(error: &clap::Error) -> Checked<Diagnostic> {
    let refusal = interpret(error).refused_because("a parse failure is not an outcome")?;
    refusal
        .diagnostics()
        .first()
        .cloned()
        .required_because("the refusal carries a diagnostic")
}

/// 診断が持つ説明の引数を、名前で取り出す。
fn described(diagnostic: &Diagnostic, key: &str) -> Checked<String> {
    diagnostic
        .description
        .args
        .iter()
        .find(|(name, _)| *name == key)
        .map(|(_, value)| value.clone())
        .required_because(&format!("the description carries {key}"))
}

#[test]
fn arguments_that_cannot_be_used_together_are_both_named() -> Checked {
    let error = conflicting_parser()
        .try_get_matches_from(["probe", "--one", "--two"])
        .refused_because("the two options are declared as conflicting")?;
    let diagnostic = diagnostic_for(&error)?;
    assert_eq!(diagnostic.id, ErrorId::ConflictingArguments);
    assert_eq!(diagnostic.description.id, "error-conflicting-arguments");
    assert_eq!(
        diagnostic.description.args,
        vec![("arguments", "--one, --two".to_string())],
        "both sides of the conflict appear in one line"
    );
    Ok(())
}

/// 相手を伴わない衝突でも、名指しできる引数だけは示す。
#[test]
fn a_conflict_reported_without_a_second_argument_names_the_one_it_has() -> Checked {
    let error = conflicting_parser()
        .try_get_matches_from(["probe", "--one", "--alone"])
        .refused_because("an exclusive option cannot be combined")?;
    let diagnostic = diagnostic_for(&error)?;
    assert_eq!(diagnostic.id, ErrorId::ConflictingArguments);
    assert_eq!(
        diagnostic.description.args,
        vec![("arguments", "--alone".to_string())]
    );
    Ok(())
}

/// 先に来た引数しか名指されない衝突は、その引数だけを並べる。
#[test]
fn a_conflict_reported_only_with_earlier_arguments_lists_those() -> Checked {
    let error = conflicting_parser()
        .try_get_matches_from(["probe", "--one", "sub"])
        .refused_because("the option and the subcommand cannot be combined")?;
    let diagnostic = diagnostic_for(&error)?;
    assert_eq!(diagnostic.id, ErrorId::ConflictingArguments);
    assert_eq!(
        diagnostic.description.args,
        vec![("arguments", "--one".to_string())]
    );
    Ok(())
}

/// 写像していない失敗も、libraryの英語をそのまま出さずに一般的な診断へ落とす。
#[test]
fn a_failure_the_mapping_does_not_name_becomes_the_general_usage_error() -> Checked {
    // 値を取らないoptionへ値を書いた場合、libraryは専用の種類で拒否する。
    let error = parse_argv(&["apply", "owner/repository", "--files=yes"], tty())
        .refused_because("a flag accepts no value")?;
    let diagnostic = error
        .diagnostics()
        .first()
        .required_because("the refusal carries a diagnostic")?;
    assert_eq!(diagnostic.id, ErrorId::InvalidArguments);
    assert_eq!(diagnostic.description.id, "error-invalid-arguments");
    assert!(
        diagnostic.description.args.is_empty(),
        "the general error names nothing it did not observe: {:?}",
        diagnostic.description.args
    );
    Ok(())
}

/// 値の誤りは、どのoptionのどの値かを両方示す。
#[test]
fn an_unusable_value_names_both_the_option_and_the_value() -> Checked {
    // 組み込みlocaleにならないtagを使う。
    let error = parse_argv(&["--lang", "zz", "ls"], tty())
        .refused_because("zz is not a display language")?;
    let diagnostic = error
        .diagnostics()
        .first()
        .required_because("the refusal carries a diagnostic")?;
    assert_eq!(diagnostic.id, ErrorId::InvalidValue);
    assert_eq!(diagnostic.description.id, "error-invalid-value");
    assert!(
        described(diagnostic, "argument")?.starts_with("--lang"),
        "the option is named: {:?}",
        diagnostic.description.args
    );
    assert_eq!(described(diagnostic, "value")?, "zz");
    Ok(())
}

/// usageが取れた場合も、引数の一覧はhelpにしかない。両方を示す。
#[test]
fn a_parse_failure_offers_the_usage_line_and_the_help_command() -> Checked {
    let error =
        parse_argv(&["ls", "--nope"], tty()).refused_because("unknown options are refused")?;
    let diagnostic = error
        .diagnostics()
        .first()
        .required_because("the refusal carries a diagnostic")?;
    let remediation = diagnostic
        .remediation
        .clone()
        .required_because("a usage error explains how to recover")?;

    let explained: Vec<&str> = remediation
        .explanation
        .iter()
        .map(|message| message.id)
        .collect();
    assert_eq!(explained, vec!["usage-hint", "remediation-run-help"]);

    let usage = remediation
        .explanation
        .first()
        .and_then(|message| {
            message
                .args
                .iter()
                .find(|(name, _)| *name == "usage")
                .map(|(_, value)| value.clone())
        })
        .required_because("the usage hint carries the usage line")?;
    assert!(
        usage.starts_with("Usage: sbxm ls"),
        "the usage of the command that failed is quoted: {usage}"
    );
    assert_eq!(usage.trim(), usage, "the quoted usage has no stray blanks");

    let commands: Vec<&str> = remediation
        .commands
        .iter()
        .map(crate::design::text::CommandLine::as_str)
        .collect();
    assert_eq!(commands, vec!["sbxm --help"]);
    Ok(())
}

/// usageを持たない失敗でも、helpへの案内は残す。
#[test]
fn a_failure_without_a_usage_line_still_offers_help() -> Checked {
    let error = parse_argv(&["--lang", "zz", "ls"], tty())
        .refused_because("zz is not a display language")?;
    let diagnostic = error
        .diagnostics()
        .first()
        .required_because("the refusal carries a diagnostic")?;
    let remediation = diagnostic
        .remediation
        .clone()
        .required_because("a usage error explains how to recover")?;
    let explained: Vec<&str> = remediation
        .explanation
        .iter()
        .map(|message| message.id)
        .collect();
    assert_eq!(explained, vec!["remediation-run-help"]);
    Ok(())
}
