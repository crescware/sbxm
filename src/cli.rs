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

use crate::error::{Diagnostic, Error, ErrorId, Msg, Result, fail};
use crate::i18n::{Catalog, Locale};
use crate::msg;
use crate::project::ProjectId;

use crate::metadata::{MAX_WORKTREES, MIN_WORKTREES};

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

/// `apply`の引数。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyArgs {
    pub project: ProjectId,
    /// global configが宣言するfileを再配置する。
    pub files: bool,
    /// managed worktreeの目標本数。
    pub worktrees: Option<u32>,
}

/// 実行するcommand。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Init(InitMode),
    Add(AddArgs),
    Apply(ApplyArgs),
    Prepare(ProjectId),
    Rebuild(ProjectId),
    Open(Option<ProjectId>),
    Stop(Vec<ProjectId>),
    Ls,
    Status(StatusScope),
    Destroy(DestroyArgs),
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

    let apply = ClapCommand::new("apply")
        .about(text("cli-apply-about")?)
        .disable_help_flag(true)
        .arg(help_flag(text("cli-help-help")?))
        .arg(
            Arg::new("project")
                .required(true)
                .value_name(project_value_name)
                .help(text("cli-apply-project-help")?),
        )
        .arg(
            Arg::new("files")
                .long("files")
                .action(ArgAction::SetTrue)
                .help(text("cli-apply-files-help")?),
        )
        .arg(
            Arg::new("worktrees")
                .long("worktrees")
                .value_name("N")
                .value_parser(clap::value_parser!(u32))
                .help(text("cli-apply-worktrees-help")?),
        );

    let prepare = ClapCommand::new("prepare")
        .about(text("cli-prepare-about")?)
        .disable_help_flag(true)
        .arg(help_flag(text("cli-help-help")?))
        .arg(
            Arg::new("project")
                .required(true)
                .value_name("owner/repository")
                .help(text("cli-prepare-project-help")?),
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
        .subcommand(apply)
        .subcommand(prepare)
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
        "apply" => Ok(Command::Apply(apply_args(matches)?)),
        "prepare" => Ok(Command::Prepare(required_project(matches)?)),
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

/// `apply`は適用する対象の明示を必須とする。省略した対象へは触れないため、何も
/// 指定しない実行は何をするか決まらない。
fn apply_args(matches: &ArgMatches) -> Result<ApplyArgs> {
    let files = matches.get_flag("files");
    let worktrees = matches.get_one::<u32>("worktrees").copied();
    if !files && worktrees.is_none() {
        return fail(
            ErrorId::ApplyScopeRequired,
            msg!("error-apply-scope-required"),
        );
    }
    Ok(ApplyArgs {
        project: required_project(matches)?,
        files,
        worktrees,
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

#[cfg(test)]
#[path = "cli_test.rs"]
mod cli_test;
