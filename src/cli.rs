//! CLI parse。
//!
//! CLI parser libraryの自動help・自動終了へlocale決定を委ねず、選択したlocaleで
//! help、usage、parse errorを生成する。validationは次の順で行う。
//!
//! 1. syntaxとoption関係
//! 2. `--lang`
//! 3. command固有の引数
//! 4. config load
//! 5. project解決
//! 6. 外部command
//! 7. mutation
//!
//! 本moduleは1から3までを担当し、config、filesystem、外部状態には触れない。

use std::sync::OnceLock;

use clap::builder::PossibleValuesParser;
use clap::error::{ContextKind, ContextValue, ErrorKind};
use clap::{Arg, ArgAction, ArgMatches, Command as ClapCommand};

use crate::compatibility;
use crate::error::{Diagnostic, Error, ErrorId, Msg, Result, fail};
use crate::i18n::{Catalog, Locale};
use crate::msg;
use crate::project::ProjectId;

/// managed worktreeの下限と上限。
pub const MIN_WORKTREES: u32 = 1;
pub const MAX_WORKTREES: u32 = 32;

/// 対話可能性。project省略時の規則へ使う。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Interactivity {
    pub stdin_is_tty: bool,
    pub stderr_is_tty: bool,
}

impl Interactivity {
    /// 選択promptはstdinから読み、stderrへ表示する。両方がTTYでなければ使えない。
    pub fn can_prompt(&self) -> bool {
        self.stdin_is_tty && self.stderr_is_tty
    }

    pub fn detect() -> Interactivity {
        use std::io::IsTerminal;
        Interactivity {
            stdin_is_tty: std::io::stdin().is_terminal(),
            stderr_is_tty: std::io::stderr().is_terminal(),
        }
    }
}

/// `init`の2 mode。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InitMode {
    /// 3 optionを1つも指定しない。
    Interactive,
    /// 3 optionをすべて指定する。
    Options {
        base_path: String,
        git_user_name: String,
        git_user_email: String,
    },
}

/// `add`の目標構成。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddArgs {
    pub project: ProjectId,
    pub worktrees: Option<u32>,
    pub detach: Option<String>,
}

/// `status`のscope。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatusScope {
    Global,
    Project(ProjectId),
}

/// `destroy`の対象と mode。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DestroyArgs {
    /// force modeはTTYかどうかにかかわらずproject引数の完全指定を必須とする。
    pub project: Option<ProjectId>,
    pub force: bool,
}

/// 実行するcommand。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Init(InitMode),
    Add(AddArgs),
    SyncFiles(ProjectId),
    Rebuild(ProjectId),
    Open(Option<ProjectId>),
    Stop(Vec<ProjectId>),
    Ls,
    Status(StatusScope),
    Destroy(DestroyArgs),
}

impl Command {
    /// 診断へ表示するcommand名。翻訳しない。
    pub fn name(&self) -> &'static str {
        match self {
            Command::Init(_) => "sbxm init",
            Command::Add(_) => "sbxm add",
            Command::SyncFiles(_) => "sbxm sync-files",
            Command::Rebuild(_) => "sbxm rebuild",
            Command::Open(_) => "sbxm open",
            Command::Stop(_) => "sbxm stop",
            Command::Ls => "sbxm ls",
            Command::Status(_) => "sbxm status",
            Command::Destroy(_) => "sbxm destroy",
        }
    }
}

/// parse結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// helpをstdoutへ出してexit code `0`。
    Help(String),
    /// version文字列をstdoutへ出してexit code `0`。
    Version(String),
    Run(Command),
}

/// argvから先読みした`--lang`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PeekedLang {
    Absent,
    Valid(Locale),
    Invalid(String),
}

/// helpとusageを構築する前に、argvから`--lang`だけを副作用なく先読みする。
///
/// locale選択だけに使用し、ほかのargument validationやcommand実行を行わない。
pub fn peek_lang(argv: &[String]) -> PeekedLang {
    let mut arguments = argv.iter().skip(1);
    while let Some(argument) = arguments.next() {
        if argument == "--" {
            break;
        }
        let value = if let Some(value) = argument.strip_prefix("--lang=") {
            Some(value.to_string())
        } else if argument == "--lang" {
            // 値が続かない場合はusage errorであり、その判定はparserへ委ねる。
            arguments.next().cloned()
        } else {
            None
        };
        if let Some(value) = value {
            return match Locale::parse_exact(&value) {
                Some(locale) => PeekedLang::Valid(locale),
                None => PeekedLang::Invalid(value),
            };
        }
    }
    PeekedLang::Absent
}

/// 組み込みlocaleのtag。`--lang`が受け付ける値と一致する。
fn supported_tags() -> Vec<&'static str> {
    Locale::ALL
        .iter()
        .map(|locale| locale.as_str())
        .collect::<Vec<_>>()
}

/// helpと診断へ並べる、受け付けるlocale tagの一覧。
fn supported_tag_list() -> String {
    supported_tags().join(", ")
}

/// `--lang`のvalue name。CLI parser libraryが`&'static str`を要求するため一度だけ組む。
fn supported_value_name() -> &'static str {
    static VALUE_NAME: OnceLock<String> = OnceLock::new();
    VALUE_NAME
        .get_or_init(|| supported_tags().join("|"))
        .as_str()
}

/// `--lang`の不正値に対するerror。configを読まずに表示する。
pub fn invalid_lang_error(value: &str) -> Error {
    Error::new(
        ErrorId::InvalidLang,
        msg!(
            "error-invalid-lang",
            value = value,
            supported = supported_tag_list()
        ),
    )
}

/// localeを決めたcatalogでargvをparseする。
pub fn parse(argv: &[String], catalog: &Catalog, interactivity: Interactivity) -> Result<Outcome> {
    let command = build_command(catalog)?;
    let matches = match command.try_get_matches_from(argv) {
        Ok(matches) => matches,
        Err(error) => return interpret_clap_error(error, catalog),
    };
    let (name, sub) = matches
        .subcommand()
        .ok_or_else(|| Error::new(ErrorId::MissingSubcommand, msg!("error-missing-subcommand")))?;
    Ok(Outcome::Run(build_invocation(name, sub, interactivity)?))
}

fn interpret_clap_error(error: clap::Error, catalog: &Catalog) -> Result<Outcome> {
    match error.kind() {
        // helpとversionはexit code `0`。libraryの既定exit codeは透過しない。
        ErrorKind::DisplayHelp | ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand => {
            Ok(Outcome::Help(error.render().to_string()))
        }
        ErrorKind::DisplayVersion => Ok(Outcome::Version(version_line())),
        _ => Err(map_parse_error(&error, catalog)),
    }
}

/// `--version`が表示する文字列。
pub fn version_line() -> String {
    format!("sbxm {}", env!("CARGO_PKG_VERSION"))
}

fn context_string(error: &clap::Error, kind: ContextKind) -> Option<String> {
    match error.get(kind) {
        Some(ContextValue::String(value)) => Some(value.clone()),
        Some(ContextValue::Strings(values)) => Some(values.join(", ")),
        Some(ContextValue::StyledStr(value)) => Some(value.to_string()),
        Some(ContextValue::StyledStrs(values)) => Some(
            values
                .iter()
                .map(|value| value.to_string())
                .collect::<Vec<_>>()
                .join(", "),
        ),
        _ => None,
    }
}

/// CLI parserのerrorを、翻訳した説明と安定したerror IDへ写像する。
fn map_parse_error(error: &clap::Error, _catalog: &Catalog) -> Error {
    let invalid_arg = context_string(error, ContextKind::InvalidArg);
    let invalid_value = context_string(error, ContextKind::InvalidValue);
    let usage = context_string(error, ContextKind::Usage);

    let (id, description) = match error.kind() {
        ErrorKind::UnknownArgument => (
            ErrorId::UnknownArgument,
            msg!(
                "error-unknown-argument",
                argument = invalid_arg.clone().unwrap_or_default()
            ),
        ),
        ErrorKind::InvalidValue | ErrorKind::ValueValidation => (
            ErrorId::InvalidValue,
            msg!(
                "error-invalid-value",
                argument = invalid_arg.clone().unwrap_or_default(),
                value = invalid_value.clone().unwrap_or_default()
            ),
        ),
        ErrorKind::InvalidSubcommand => (
            ErrorId::UnknownSubcommand,
            msg!(
                "error-unknown-subcommand",
                subcommand =
                    context_string(error, ContextKind::InvalidSubcommand).unwrap_or_default()
            ),
        ),
        ErrorKind::MissingRequiredArgument => (
            ErrorId::MissingRequiredArgument,
            msg!(
                "error-missing-required-argument",
                argument = context_string(error, ContextKind::InvalidArg).unwrap_or_default()
            ),
        ),
        ErrorKind::MissingSubcommand => {
            (ErrorId::MissingSubcommand, msg!("error-missing-subcommand"))
        }
        ErrorKind::ArgumentConflict => (
            ErrorId::ConflictingArguments,
            msg!(
                "error-conflicting-arguments",
                arguments = context_string(error, ContextKind::PriorArg)
                    .map(|prior| match &invalid_arg {
                        Some(invalid) => format!("{invalid}, {prior}"),
                        None => prior,
                    })
                    .or_else(|| invalid_arg.clone())
                    .unwrap_or_default()
            ),
        ),
        _ => (ErrorId::InvalidArguments, msg!("error-invalid-arguments")),
    };

    let mut diagnostic = Diagnostic::new(id, description);
    if let Some(usage) = usage {
        diagnostic = diagnostic.remediation(msg!("usage-hint", usage = usage.trim()));
    } else {
        diagnostic = diagnostic.remediation(msg!("remediation-run-help", command = "sbxm --help"));
    }
    Error::single(diagnostic)
}

/// FTLからhelp textを組み立てたparserを作る。
fn build_command(catalog: &Catalog) -> Result<ClapCommand> {
    let message = |message: &Msg| -> Result<String> {
        catalog.format(message).map_err(|failure| {
            Error::new(
                ErrorId::MessageFormatFailed,
                msg!("error-invalid-arguments").with("detail", failure),
            )
        })
    };
    let text = |id: &'static str| -> Result<String> { message(&msg!(id)) };

    let usage_heading = text("cli-heading-usage")?;
    let commands_heading = text("cli-heading-commands")?;
    let options_heading = text("cli-heading-options")?;
    let arguments_heading = text("cli-heading-arguments")?;

    let root_template = format!(
        "{{about}}\n\n{usage_heading} {{usage}}\n\n{commands_heading}\n{{subcommands}}\n\n{options_heading}\n{{options}}"
    );
    let leaf_template =
        format!("{{about}}\n\n{usage_heading} {{usage}}\n\n{options_heading}\n{{options}}");
    let positional_template = format!(
        "{{about}}\n\n{usage_heading} {{usage}}\n\n{arguments_heading}\n{{positionals}}\n\n{options_heading}\n{{options}}"
    );

    // clapはrequiredなvalueを`<>`、optionalなvalueを`[]`で囲む。どちらの表示でも
    // 読めるよう、value name自体には囲み記号を含めない。
    let project_value_name = "owner/repository";

    // 受け付ける値も表示も組み込みlocaleの定義から導出する。言語を増やしても触らない。
    let lang = Arg::new("lang")
        .long("lang")
        .value_name(supported_value_name())
        .global(true)
        .value_parser(PossibleValuesParser::new(supported_tags()))
        // 値の一覧はFTLのhelp textに含めるため、libraryの英語固定表記は出さない。
        .hide_possible_values(true)
        .display_order(900)
        .help(message(&msg!(
            "cli-lang-help",
            supported = supported_tag_list()
        ))?);

    // helpとversionは、commandごとのoptionより後に並べる。
    let help_flag = |catalog_help: String| {
        Arg::new("help")
            .long("help")
            .short('h')
            .action(ArgAction::Help)
            .display_order(1000)
            .help(catalog_help)
    };

    let init = ClapCommand::new("init")
        .about(text("cli-init-about")?)
        .help_template(leaf_template.clone())
        .disable_help_flag(true)
        .arg(help_flag(text("cli-help-help")?))
        .arg(
            Arg::new("base-path")
                .long("base-path")
                .value_name("PATH")
                .help(text("cli-init-base-path-help")?),
        )
        .arg(
            Arg::new("git-user-name")
                .long("git-user-name")
                .value_name("NAME")
                .help(text("cli-init-git-user-name-help")?),
        )
        .arg(
            Arg::new("git-user-email")
                .long("git-user-email")
                .value_name("EMAIL")
                .help(text("cli-init-git-user-email-help")?),
        );

    let add = ClapCommand::new("add")
        .about(text("cli-add-about")?)
        .help_template(positional_template.clone())
        .disable_help_flag(true)
        .arg(help_flag(text("cli-help-help")?))
        .arg(
            Arg::new("project")
                .required(true)
                .value_name(project_value_name)
                .help(text("cli-add-project-help")?),
        )
        .arg(
            Arg::new("worktrees")
                .long("worktrees")
                .value_name("N")
                .help(text("cli-add-worktrees-help")?),
        )
        .arg(
            Arg::new("detach")
                .long("detach")
                .value_name("BRANCH")
                .help(text("cli-add-detach-help")?),
        );

    let sync_files = ClapCommand::new("sync-files")
        .about(text("cli-sync-files-about")?)
        .help_template(positional_template.clone())
        .disable_help_flag(true)
        .arg(help_flag(text("cli-help-help")?))
        .arg(
            Arg::new("project")
                .required(true)
                .value_name(project_value_name)
                .help(text("cli-sync-files-project-help")?),
        );

    let rebuild = ClapCommand::new("rebuild")
        .about(text("cli-rebuild-about")?)
        .help_template(positional_template.clone())
        .disable_help_flag(true)
        .arg(help_flag(text("cli-help-help")?))
        .arg(
            Arg::new("project")
                .required(true)
                .value_name(project_value_name)
                .help(text("cli-rebuild-project-help")?),
        );

    let open = ClapCommand::new("open")
        .about(text("cli-open-about")?)
        .help_template(positional_template.clone())
        .disable_help_flag(true)
        .arg(help_flag(text("cli-help-help")?))
        .arg(
            Arg::new("project")
                .value_name(project_value_name)
                .help(text("cli-open-project-help")?),
        );

    let stop = ClapCommand::new("stop")
        .about(text("cli-stop-about")?)
        .help_template(positional_template.clone())
        .disable_help_flag(true)
        .arg(help_flag(text("cli-help-help")?))
        .arg(
            Arg::new("project")
                .value_name(project_value_name)
                .num_args(0..)
                .action(ArgAction::Append)
                .help(text("cli-stop-project-help")?),
        );

    let ls = ClapCommand::new("ls")
        .about(text("cli-ls-about")?)
        .help_template(leaf_template.clone())
        .disable_help_flag(true)
        .arg(help_flag(text("cli-help-help")?));

    let status = ClapCommand::new("status")
        .about(text("cli-status-about")?)
        .help_template(positional_template.clone())
        .disable_help_flag(true)
        .arg(help_flag(text("cli-help-help")?))
        .arg(
            Arg::new("project")
                .value_name(project_value_name)
                .help(text("cli-status-project-help")?),
        )
        .arg(
            Arg::new("global")
                .long("global")
                .short('g')
                .action(ArgAction::SetTrue)
                .help(text("cli-status-global-help")?),
        );

    let destroy = ClapCommand::new("destroy")
        .about(text("cli-destroy-about")?)
        .help_template(positional_template)
        .disable_help_flag(true)
        .arg(help_flag(text("cli-help-help")?))
        .arg(
            Arg::new("project")
                .value_name(project_value_name)
                .help(text("cli-destroy-project-help")?),
        )
        .arg(
            Arg::new("force")
                .long("force")
                .short('f')
                .action(ArgAction::SetTrue)
                .help(text("cli-destroy-force-help")?),
        );

    Ok(ClapCommand::new("sbxm")
        .about(text("cli-about")?)
        .version(env!("CARGO_PKG_VERSION"))
        .help_template(root_template)
        .disable_help_flag(true)
        .disable_version_flag(true)
        .disable_help_subcommand(true)
        .subcommand_required(true)
        .arg_required_else_help(false)
        .arg(lang)
        .arg(help_flag(text("cli-help-help")?))
        .arg(
            Arg::new("version")
                .long("version")
                .short('V')
                .action(ArgAction::Version)
                .display_order(1001)
                .help(text("cli-version-help")?),
        )
        .subcommand(init)
        .subcommand(add)
        .subcommand(sync_files)
        .subcommand(rebuild)
        .subcommand(open)
        .subcommand(stop)
        .subcommand(ls)
        .subcommand(status)
        .subcommand(destroy))
}

fn build_invocation(
    name: &str,
    matches: &ArgMatches,
    interactivity: Interactivity,
) -> Result<Command> {
    match name {
        "init" => Ok(Command::Init(init_mode(matches)?)),
        "add" => Ok(Command::Add(add_args(matches)?)),
        "sync-files" => Ok(Command::SyncFiles(required_project(matches)?)),
        "rebuild" => Ok(Command::Rebuild(required_project(matches)?)),
        "open" => Ok(Command::Open(optional_project(
            matches,
            interactivity,
            "sbxm open",
        )?)),
        "stop" => Ok(Command::Stop(stop_projects(matches, interactivity)?)),
        "ls" => Ok(Command::Ls),
        "status" => Ok(Command::Status(status_scope(matches)?)),
        "destroy" => Ok(Command::Destroy(destroy_args(matches, interactivity)?)),
        other => fail(
            ErrorId::UnknownSubcommand,
            msg!("error-unknown-subcommand", subcommand = other),
        ),
    }
}

fn init_mode(matches: &ArgMatches) -> Result<InitMode> {
    let base_path = matches.get_one::<String>("base-path").cloned();
    let git_user_name = matches.get_one::<String>("git-user-name").cloned();
    let git_user_email = matches.get_one::<String>("git-user-email").cloned();

    let provided = [&base_path, &git_user_name, &git_user_email]
        .iter()
        .filter(|value| value.is_some())
        .count();

    match provided {
        0 => Ok(InitMode::Interactive),
        3 => Ok(InitMode::Options {
            base_path: base_path.expect("checked above"),
            git_user_name: git_user_name.expect("checked above"),
            git_user_email: git_user_email.expect("checked above"),
        }),
        _ => {
            // configやfilesystemを読む前に、不足optionを表示して終了する。
            let mut missing = Vec::new();
            if base_path.is_none() {
                missing.push("--base-path");
            }
            if git_user_name.is_none() {
                missing.push("--git-user-name");
            }
            if git_user_email.is_none() {
                missing.push("--git-user-email");
            }
            fail(
                ErrorId::InitIncompleteOptions,
                msg!(
                    "error-init-incomplete-options",
                    missing = missing.join(", ")
                ),
            )
        }
    }
}

fn add_args(matches: &ArgMatches) -> Result<AddArgs> {
    let project = required_project(matches)?;
    let detach = matches.get_one::<String>("detach").cloned();

    let worktrees = match matches.get_one::<String>("worktrees") {
        Some(raw) => {
            let parsed: Option<u32> = raw.parse().ok();
            match parsed {
                Some(value) if (MIN_WORKTREES..=MAX_WORKTREES).contains(&value) => Some(value),
                _ => {
                    return fail(
                        ErrorId::WorktreesOutOfRange,
                        msg!(
                            "error-worktrees-out-of-range",
                            value = raw,
                            minimum = MIN_WORKTREES,
                            maximum = MAX_WORKTREES
                        ),
                    );
                }
            }
        }
        None => None,
    };

    // 2個以上のmanaged worktreeは、起点branchの明示を必須とする。
    if worktrees.is_some_and(|value| value >= 2) && detach.is_none() {
        return fail(
            ErrorId::WorktreesRequireDetach,
            msg!("error-worktrees-require-detach"),
        );
    }

    Ok(AddArgs {
        project,
        worktrees,
        detach,
    })
}

fn status_scope(matches: &ArgMatches) -> Result<StatusScope> {
    let global = matches.get_flag("global");
    let project = matches.get_one::<String>("project");
    match (global, project) {
        (true, None) => Ok(StatusScope::Global),
        (false, Some(value)) => Ok(StatusScope::Project(ProjectId::parse(value)?)),
        _ => fail(
            ErrorId::StatusScopeRequired,
            msg!("error-status-scope-required"),
        ),
    }
}

fn destroy_args(matches: &ArgMatches, interactivity: Interactivity) -> Result<DestroyArgs> {
    let force = matches.get_flag("force");
    let project = matches.get_one::<String>("project");
    match project {
        Some(value) => Ok(DestroyArgs {
            project: Some(ProjectId::parse(value)?),
            force,
        }),
        None if force => {
            // force modeはTTYかどうかにかかわらず完全指定を必須とする。
            fail(
                ErrorId::ProjectArgumentRequired,
                msg!(
                    "error-project-argument-required",
                    command = "sbxm destroy --force"
                ),
            )
        }
        None => {
            require_prompt_capability(interactivity, "sbxm destroy")?;
            Ok(DestroyArgs {
                project: None,
                force,
            })
        }
    }
}

fn stop_projects(matches: &ArgMatches, interactivity: Interactivity) -> Result<Vec<ProjectId>> {
    let values: Vec<String> = matches
        .get_many::<String>("project")
        .map(|values| values.cloned().collect())
        .unwrap_or_default();
    if values.is_empty() {
        require_prompt_capability(interactivity, "sbxm stop")?;
        return Ok(Vec::new());
    }
    let mut projects = Vec::with_capacity(values.len());
    for value in values {
        projects.push(ProjectId::parse(&value)?);
    }
    Ok(projects)
}

fn required_project(matches: &ArgMatches) -> Result<ProjectId> {
    let value = matches.get_one::<String>("project").ok_or_else(|| {
        Error::new(
            ErrorId::MissingRequiredArgument,
            msg!(
                "error-missing-required-argument",
                argument = "<owner/repository>"
            ),
        )
    })?;
    ProjectId::parse(value)
}

fn optional_project(
    matches: &ArgMatches,
    interactivity: Interactivity,
    command: &str,
) -> Result<Option<ProjectId>> {
    match matches.get_one::<String>("project") {
        Some(value) => Ok(Some(ProjectId::parse(value)?)),
        None => {
            require_prompt_capability(interactivity, command)?;
            Ok(None)
        }
    }
}

/// 非TTYで対象を省略した場合は、外部状態を読む前に終了する。
fn require_prompt_capability(interactivity: Interactivity, command: &str) -> Result<()> {
    if interactivity.can_prompt() {
        return Ok(());
    }
    fail(
        ErrorId::ProjectArgumentRequired,
        msg!("error-project-argument-required", command = command),
    )
}

/// Phase 1で未実装のcommandを拒否する。
pub fn not_implemented(command: &Command) -> Error {
    compatibility::not_implemented(command.name())
}

#[cfg(test)]
mod tests {
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
            peek_lang(&argv(&["--lang", "fr", "ls"])),
            PeekedLang::Invalid("fr".to_string())
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
        let error = invalid_lang_error("fr");
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
            "init",
            "add",
            "sync-files",
            "rebuild",
            "open",
            "stop",
            "ls",
            "status",
            "destroy",
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
    fn commands_that_always_need_a_project_refuse_to_prompt() {
        for name in ["add", "sync-files", "rebuild"] {
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
            let error = run(&arguments, non_tty())
                .expect_err("a non-interactive run needs an explicit target");
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
            let error = run(&["open"], interactivity)
                .expect_err("both streams must be a terminal to prompt");
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
            vec!["sync-files", "owner/repo/extra"],
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
        let error = run(&["--lang", "fr", "ls"], tty()).expect_err("fr is not supported");
        assert_eq!(error.first_id(), Some(ErrorId::InvalidValue));
    }

    #[test]
    fn unimplemented_commands_report_a_stable_error_id() {
        let error = not_implemented(&Command::Ls);
        assert_eq!(error.first_id(), Some(ErrorId::NotImplemented));
    }
}
