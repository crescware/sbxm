//! sbxm。案件ごとのDocker Sandboxを構築、接続、診断、破棄するCLI。
//!
//! 公開CLIの全commandを実装する。実行順は、引数validation、config load、
//! project解決、外部command、mutationとする。

mod archive;
mod cli;
mod command;
mod compatibility;
mod config;
mod error;
mod git;
mod hash;
mod i18n;
mod metadata;
mod paths;
mod project;
mod workflow;

use std::io::Write;
use std::process::ExitCode as ProcessExitCode;

use cli::{Command, Interactivity, Outcome, PeekedLang, StatusScope};
use command::RealHost;
use compatibility::SandboxState;
use config::{ConfigLocation, ConfigState};
use error::{Diagnostic, Error, ErrorId, ExitCode, Result};
use i18n::{Catalog, Locale, shell_locale};
use metadata::CreationMode;
use workflow::Reporter;
use workflow::destroy::TerminalConfirmPrompt;
use workflow::files::Placement;
use workflow::init::{InitRequest, TerminalPrompt};
use workflow::select::TerminalProjectPrompt;
use workflow::{
    add, destroy, inventory, ls, open, rebuild, sandbox, status_global, status_project, stop,
    sync_files,
};

fn main() -> ProcessExitCode {
    let argv: Vec<String> = std::env::args().collect();
    let code = run(&argv);
    ProcessExitCode::from(code.as_i32() as u8)
}

fn run(argv: &[String]) -> ExitCode {
    // helpとusageを構築する前に、argvから`--lang`だけを副作用なく先読みする。
    let peeked = cli::peek_lang(argv);

    let location = match ConfigLocation::discover() {
        Ok(location) => location,
        Err(error) => {
            report(&Catalog::new(Locale::En), &error);
            return error.exit_code();
        }
    };

    let display_locale = resolve_display_locale(&peeked, &location);
    let catalog = Catalog::new(display_locale);

    // `--lang`が不正な場合はconfigを読まず、shell localeまたは`en`でparse errorを表示する。
    if let PeekedLang::Invalid(value) = &peeked {
        let error = cli::invalid_lang_error(value);
        report(&catalog, &error);
        return error.exit_code();
    }

    let interactivity = Interactivity::detect();
    match cli::parse(argv, &catalog, interactivity) {
        Ok(Outcome::Help(text)) => {
            print!("{text}");
            ExitCode::Success
        }
        Ok(Outcome::Version(text)) => {
            println!("{text}");
            ExitCode::Success
        }
        Ok(Outcome::Run(command)) => {
            dispatch(&command, &location, &peeked, display_locale, interactivity)
        }
        Err(error) => {
            report(&catalog, &error);
            error.exit_code()
        }
    }
}

/// helpとusageのlocaleを決める。
///
/// 1. argvから先読みした有効な`--lang`
/// 2. read-onlyかつbest-effortで読み込めた有効なglobal configの`language`
/// 3. shell locale
/// 4. `en`
fn resolve_display_locale(peeked: &PeekedLang, location: &ConfigLocation) -> Locale {
    if let PeekedLang::Valid(locale) = peeked {
        return *locale;
    }
    // configが不在、構文不正、未知version、permission不正、symlink、read失敗のいずれでも
    // help表示自体は妨げず、shell localeへfallbackする。
    if let Ok(ConfigState::Valid { config, .. }) = config::load(location) {
        return config.language;
    }
    shell_locale().unwrap_or(Locale::En)
}

/// commandを実行し、結果を表示してexit codeを返す。
///
/// 表示localeはconfigのvalidationを経て確定するため、診断の表示までここで行う。
fn dispatch(
    command: &Command,
    location: &ConfigLocation,
    peeked: &PeekedLang,
    display_locale: Locale,
    interactivity: Interactivity,
) -> ExitCode {
    let lang_option = match peeked {
        PeekedLang::Valid(locale) => Some(*locale),
        _ => None,
    };

    match command {
        Command::Init(mode) => {
            let request = InitRequest {
                mode: mode.clone(),
                lang: lang_option,
                interactivity,
            };
            let mut prompt = TerminalPrompt;
            match workflow::init::run(location, &request, &RealHost, &mut prompt) {
                Ok(output) => {
                    let catalog = Catalog::new(output.locale);
                    let reporter = Reporter::new(&catalog);
                    let mut stderr = std::io::stderr();
                    for warning in &output.warnings {
                        reporter.print_warning(warning, &mut stderr);
                    }
                    let path = paths::display(&output.config_path);
                    let message = if output.already_initialized {
                        msg!("init-already-initialized", path = path)
                    } else {
                        msg!("init-created", path = path)
                    };
                    println!("{}", format_or_report(&catalog, &message));
                    println!("{}", text_or_report(&catalog, "init-next-step"));
                    ExitCode::Success
                }
                Err(error) => {
                    report(&Catalog::new(display_locale), &error);
                    error.exit_code()
                }
            }
        }

        Command::Status(StatusScope::Global) => {
            let catalog = match effective_catalog(location, lang_option, true) {
                Ok(catalog) => catalog,
                Err(error) => {
                    report(&Catalog::new(display_locale), &error);
                    return error.exit_code();
                }
            };
            let status = status_global::diagnose(location, &RealHost);
            let reporter = Reporter::new(&catalog);

            let table = reporter.render_status_table(
                "status-global-section",
                "status-column-item",
                "status-column-status",
                &status.rows,
            );
            print!("{table}");
            if let Some(legend) = reporter.render_legend(&status.rows) {
                print!("\n{legend}");
            }
            let _ = std::io::stdout().flush();

            let mut stderr = std::io::stderr();
            for warning in &status.warnings {
                reporter.print_warning(warning, &mut stderr);
            }
            for diagnostic in &status.diagnostics {
                reporter.print_error(&Error::single(diagnostic.clone()), &mut stderr);
            }

            if status.is_healthy() {
                ExitCode::Success
            } else {
                ExitCode::Failure
            }
        }

        Command::Status(StatusScope::Project(project)) => {
            let (config, catalog) = match require_config(location, lang_option) {
                Ok(pair) => pair,
                Err(error) => {
                    report(&Catalog::new(display_locale), &error);
                    return error.exit_code();
                }
            };
            match status_project::diagnose(
                &config,
                project,
                &RealHost,
                std::path::Path::new(sandbox::WORKSPACE_ROOT),
            ) {
                Ok(status) => print_project_status(&catalog, &status),
                Err(error) => {
                    report(&catalog, &error);
                    error.exit_code()
                }
            }
        }

        Command::Add(arguments) => {
            let (config, catalog) = match require_config(location, lang_option) {
                Ok(pair) => pair,
                Err(error) => {
                    report(&Catalog::new(display_locale), &error);
                    return error.exit_code();
                }
            };
            let request = add::AddRequest {
                project: arguments.project.clone(),
                worktrees: arguments.worktrees,
                detach: arguments.detach.clone(),
            };
            match add::run(
                &config,
                location,
                &request,
                &RealHost,
                std::path::Path::new(sandbox::WORKSPACE_ROOT),
            ) {
                Ok(output) => {
                    print_add_output(&catalog, &output);
                    ExitCode::Success
                }
                Err(error) => {
                    report(&catalog, &error);
                    error.exit_code()
                }
            }
        }

        Command::SyncFiles(project) => {
            let (config, catalog) = match require_config(location, lang_option) {
                Ok(pair) => pair,
                Err(error) => {
                    report(&Catalog::new(display_locale), &error);
                    return error.exit_code();
                }
            };
            match sync_files::run(
                &config,
                project,
                &RealHost,
                std::path::Path::new(sandbox::WORKSPACE_ROOT),
            ) {
                Ok(output) => {
                    print_sync_output(&catalog, &output);
                    ExitCode::Success
                }
                Err(error) => {
                    report(&catalog, &error);
                    error.exit_code()
                }
            }
        }

        Command::Open(project) => {
            let (config, catalog) = match require_config(location, lang_option) {
                Ok(pair) => pair,
                Err(error) => {
                    report(&Catalog::new(display_locale), &error);
                    return error.exit_code();
                }
            };
            let mut prompt = TerminalProjectPrompt {
                heading: "select-open-heading",
                locale: catalog.locale(),
            };
            let prepared = match open::prepare(
                &config,
                location,
                project.as_ref(),
                &RealHost,
                &mut prompt,
                std::path::Path::new(sandbox::WORKSPACE_ROOT),
                inventory::Poll::default(),
            ) {
                Ok(prepared) => prepared,
                Err(error) => {
                    report(&catalog, &error);
                    return error.exit_code();
                }
            };

            // 接続先はterminalを引き渡す前に見せる。
            let mut stderr = std::io::stderr();
            let reporter = Reporter::new(&catalog);
            for warning in &prepared.warnings {
                reporter.print_warning(warning, &mut stderr);
            }
            let _ = writeln!(
                stderr,
                "{}",
                format_or_report(
                    &catalog,
                    &msg!(
                        "open-connecting",
                        project = prepared.project,
                        sandbox = prepared.sandbox
                    )
                )
            );
            if !prepared.worktrees.is_empty() {
                let _ = writeln!(stderr, "{}", text_or_report(&catalog, "open-worktrees"));
                for worktree in &prepared.worktrees {
                    let _ = writeln!(stderr, "  {worktree}");
                }
            }

            match open::connect(&RealHost, &prepared) {
                Ok(()) => ExitCode::Success,
                Err(error) => {
                    report(&catalog, &error);
                    error.exit_code()
                }
            }
        }

        Command::Stop(projects) => {
            let (config, catalog) = match require_config(location, lang_option) {
                Ok(pair) => pair,
                Err(error) => {
                    report(&Catalog::new(display_locale), &error);
                    return error.exit_code();
                }
            };
            let mut prompt = TerminalProjectPrompt {
                heading: "select-stop-heading",
                locale: catalog.locale(),
            };
            match stop::run(
                &config,
                projects,
                &RealHost,
                &mut prompt,
                std::path::Path::new(sandbox::WORKSPACE_ROOT),
                inventory::Poll::default(),
            ) {
                Ok(report) => print_stop_report(&catalog, &report),
                Err(error) => {
                    report(&catalog, &error);
                    error.exit_code()
                }
            }
        }

        Command::Rebuild(project) => {
            let (config, catalog) = match require_config(location, lang_option) {
                Ok(pair) => pair,
                Err(error) => {
                    report(&Catalog::new(display_locale), &error);
                    return error.exit_code();
                }
            };
            match rebuild::run(
                &config,
                location,
                project,
                &RealHost,
                std::path::Path::new(sandbox::WORKSPACE_ROOT),
                inventory::Poll::default(),
            ) {
                Ok(output) => {
                    let reporter = Reporter::new(&catalog);
                    let mut stderr = std::io::stderr();
                    for warning in &output.warnings {
                        reporter.print_warning(warning, &mut stderr);
                    }
                    let message = if output.unchanged {
                        msg!("rebuild-unchanged", project = output.project)
                    } else {
                        msg!(
                            "rebuild-applied",
                            project = output.project,
                            sandbox = output.sandbox,
                            generation = crate::hash::short_hex(&output.applied)
                        )
                    };
                    println!("{}", format_or_report(&catalog, &message));
                    ExitCode::Success
                }
                Err(error) => {
                    report(&catalog, &error);
                    error.exit_code()
                }
            }
        }

        Command::Destroy(arguments) => {
            let (config, catalog) = match require_config(location, lang_option) {
                Ok(pair) => pair,
                Err(error) => {
                    report(&Catalog::new(display_locale), &error);
                    return error.exit_code();
                }
            };
            let mut prompt = TerminalProjectPrompt {
                heading: "select-destroy-heading",
                locale: catalog.locale(),
            };
            let prepared = match destroy::prepare(
                &config,
                arguments.project.as_ref(),
                arguments.force,
                &RealHost,
                &mut prompt,
                std::path::Path::new(sandbox::WORKSPACE_ROOT),
            ) {
                Ok(prepared) => prepared,
                Err(error) => {
                    report(&catalog, &error);
                    return error.exit_code();
                }
            };

            print_destroy_plan(&catalog, &prepared.plan);
            let mut confirm = TerminalConfirmPrompt {
                locale: catalog.locale(),
            };
            if let Err(error) =
                destroy::confirm(&prepared, interactivity.can_prompt(), &mut confirm)
            {
                report(&catalog, &error);
                return error.exit_code();
            }

            match destroy::execute(&RealHost, &prepared, inventory::Poll::default()) {
                Ok(outcome) => {
                    let reporter = Reporter::new(&catalog);
                    let mut stderr = std::io::stderr();
                    for warning in &outcome.warnings {
                        reporter.print_warning(warning, &mut stderr);
                    }
                    println!(
                        "{}",
                        format_or_report(
                            &catalog,
                            &msg!("destroy-done", project = outcome.project)
                        )
                    );
                    println!(
                        "{}",
                        format_or_report(
                            &catalog,
                            &msg!("destroy-re-register", command = outcome.re_register)
                        )
                    );
                    ExitCode::Success
                }
                Err(error) => {
                    report(&catalog, &error);
                    error.exit_code()
                }
            }
        }

        Command::Ls => {
            let (config, catalog) = match require_config(location, lang_option) {
                Ok(pair) => pair,
                Err(error) => {
                    report(&Catalog::new(display_locale), &error);
                    return error.exit_code();
                }
            };
            match ls::run(
                &config,
                &RealHost,
                std::path::Path::new(sandbox::WORKSPACE_ROOT),
            ) {
                Ok(listing) => {
                    print_listing(&catalog, &listing);
                    ExitCode::Success
                }
                Err(error) => {
                    report(&catalog, &error);
                    error.exit_code()
                }
            }
        }
    }
}

/// `add`の成功出力。
fn print_add_output(catalog: &Catalog, output: &add::AddOutput) {
    let reporter = Reporter::new(catalog);
    let mut stderr = std::io::stderr();
    for warning in &output.warnings {
        reporter.print_warning(warning, &mut stderr);
    }

    if output.already_built {
        println!(
            "{}",
            format_or_report(
                catalog,
                &msg!("add-already-built", project = output.project)
            )
        );
    }

    print!(
        "{}",
        reporter.render_fields(&[
            ("add-field-project", output.project.clone()),
            ("add-field-sandbox", output.sandbox.clone()),
            ("add-field-creation-mode", output.mode.to_string()),
            ("add-field-start-branch", output.start_ref.clone()),
            (
                "add-field-managed-worktrees",
                output.worktrees.len().to_string()
            ),
            ("add-field-host-clone", paths::display(&output.host_clone)),
            (
                "add-field-sandbox-state",
                sandbox_state(output.sandbox_state).to_string()
            ),
        ])
    );

    let worktrees: Vec<Vec<String>> = output
        .worktrees
        .iter()
        .map(|worktree| {
            vec![
                worktree.path.clone(),
                worktree.created_from.clone(),
                worktree.head.clone().unwrap_or_else(|| "-".to_string()),
                worktree.mode.to_string(),
            ]
        })
        .collect();
    if !worktrees.is_empty() {
        print!(
            "\n{}",
            reporter.render_value_table(
                &[
                    "column-worktree",
                    "column-created-from",
                    "column-head",
                    "column-mode"
                ],
                &worktrees,
            )
        );
    }

    let files: Vec<Vec<String>> = output
        .files
        .iter()
        .map(|file| {
            vec![
                paths::display(&file.source),
                file.destination.clone(),
                file.placement.as_str().to_string(),
            ]
        })
        .collect();
    if !files.is_empty() {
        print!(
            "\n{}",
            reporter.render_value_table(
                &["column-file", "column-destination", "column-result"],
                &files,
            )
        );
        println!("{}", text_or_report(catalog, "files-secret-hint"));
    }

    if !output.mise_candidates.is_empty() {
        println!("\n{}", text_or_report(catalog, "add-mise-heading"));
        for candidate in &output.mise_candidates {
            println!("  {candidate}");
        }
        println!("{}", text_or_report(catalog, "add-mise-hint"));
    }

    let mut values: Vec<(&str, &str)> = vec![(
        sandbox_state(output.sandbox_state),
        sandbox_state_legend(output.sandbox_state),
    )];
    for worktree in &output.worktrees {
        values.push(mode_legend(worktree.mode));
    }
    for file in &output.files {
        values.push(placement_legend(file.placement));
    }
    if let Some(legend) = reporter.render_value_legend(&values) {
        print!("\n{legend}");
    }
    let _ = std::io::stdout().flush();
}

/// project scopeの`status`の出力。
///
/// 指定案件だけを診断し、global環境の検査結果を混ぜない。
fn print_project_status(catalog: &Catalog, status: &status_project::ProjectStatus) -> ExitCode {
    let reporter = Reporter::new(catalog);

    let mut fields: Vec<(&str, String)> = vec![("status-item-project", status.project.clone())];
    fields.extend(
        status
            .items
            .iter()
            .map(|item| (item.item, item.value.as_str().to_string())),
    );
    println!("{}", text_or_report(catalog, "status-project-section"));
    print!(
        "{}",
        reporter.render_value_table(
            &["status-column-item", "status-column-value"],
            &fields
                .iter()
                .map(|(item, value)| vec![text_or_report(catalog, item), value.clone()])
                .collect::<Vec<_>>(),
        )
    );

    // 0件でもsectionとheaderを出す。表がないことと、観測できなかったことは別である。
    let rows: Vec<Vec<String>> = status
        .worktrees
        .iter()
        .map(|worktree| {
            vec![
                worktree.path.clone(),
                worktree.kind.to_string(),
                worktree.mode.as_str().to_string(),
                worktree.state.as_str().to_string(),
            ]
        })
        .collect();
    println!("\n{}", text_or_report(catalog, "status-worktrees-section"));
    print!(
        "{}",
        reporter.render_value_table(
            &["column-path", "column-kind", "column-mode", "column-state"],
            &rows,
        )
    );

    let mut values: Vec<(&str, &str)> = status
        .items
        .iter()
        .map(|item| (item.value.as_str(), item.value.legend_id()))
        .collect();
    for worktree in &status.worktrees {
        values.push((worktree.mode.as_str(), worktree.mode.legend_id()));
        values.push((worktree.state.as_str(), worktree.state.legend_id()));
    }
    if let Some(legend) = reporter.render_value_legend(&values) {
        print!("\n{legend}");
    }
    let _ = std::io::stdout().flush();

    let mut stderr = std::io::stderr();
    for diagnostic in &status.diagnostics {
        reporter.print_error(&Error::single(diagnostic.clone()), &mut stderr);
    }
    if status.is_healthy() {
        ExitCode::Success
    } else {
        ExitCode::Failure
    }
}

/// 削除対象・保持対象の1行。pathはそのまま、それ以外は説明を訳す。
fn destroy_target(catalog: &Catalog, target: &destroy::Target) -> String {
    match target {
        destroy::Target::Path(path) => path.clone(),
        destroy::Target::Described(message) => format_or_report(catalog, message),
    }
}

/// `destroy`が何を消し、何を残すかを削除前に見せる。
fn print_destroy_plan(catalog: &Catalog, plan: &destroy::DestroyPlan) {
    let reporter = Reporter::new(catalog);
    let mut stderr = std::io::stderr();

    print!(
        "{}",
        reporter.render_fields(&[
            ("add-field-project", plan.project.clone()),
            ("add-field-sandbox", plan.sandbox.clone()),
            ("add-field-sandbox-state", plan.state.as_str().to_string()),
        ])
    );

    if plan.force {
        let _ = writeln!(
            stderr,
            "{}",
            text_or_report(catalog, "destroy-force-notice")
        );
    } else if !plan.worktrees.is_empty() {
        let rows: Vec<Vec<String>> = plan
            .worktrees
            .iter()
            .map(|worktree| {
                vec![
                    worktree.relative.clone(),
                    worktree.kind.as_str().to_string(),
                    worktree.mode.as_str().to_string(),
                    worktree.branch.clone().unwrap_or_else(|| "-".to_string()),
                    worktree.head.clone(),
                    worktree.remote.as_str().to_string(),
                ]
            })
            .collect();
        println!("\n{}", text_or_report(catalog, "status-worktrees-section"));
        print!(
            "{}",
            reporter.render_value_table(
                &[
                    "column-path",
                    "column-kind",
                    "column-mode",
                    "column-branch",
                    "column-head",
                    "column-remote"
                ],
                &rows,
            )
        );
        // 状態値は翻訳しないため、正本locale以外では説明を添える。
        let values: Vec<(&str, &str)> = plan
            .worktrees
            .iter()
            .flat_map(|worktree| worktree.legends())
            .collect();
        if let Some(legend) = reporter.render_value_legend(&values) {
            print!("\n{legend}");
        }
    }

    println!("\n{}", text_or_report(catalog, "destroy-removes"));
    for target in &plan.removes {
        println!("  {}", destroy_target(catalog, target));
    }
    println!("{}", text_or_report(catalog, "destroy-keeps"));
    for target in &plan.keeps {
        println!("  {}", destroy_target(catalog, target));
    }
    // 消す前に、消したあと元へ戻す方法まで見せる。
    println!(
        "\n{}",
        format_or_report(
            catalog,
            &msg!("destroy-re-register", command = plan.re_register.clone())
        )
    );
    let _ = std::io::stdout().flush();
}

/// `stop`の出力。
///
/// 対象ごとの結果を表示し、1件でも失敗していればexit code `1`とする。
fn print_stop_report(catalog: &Catalog, report: &stop::StopReport) -> ExitCode {
    let reporter = Reporter::new(catalog);
    let rows: Vec<Vec<String>> = report
        .outcomes
        .iter()
        .map(|outcome| {
            vec![
                outcome.project.clone(),
                outcome.sandbox.clone(),
                outcome.result.as_str().to_string(),
            ]
        })
        .collect();
    print!(
        "{}",
        reporter.render_value_table(
            &["column-project", "column-sandbox", "column-result"],
            &rows,
        )
    );

    let values: Vec<(&str, &str)> = report
        .outcomes
        .iter()
        .map(|outcome| (outcome.result.as_str(), outcome.result.legend_id()))
        .collect();
    if let Some(legend) = reporter.render_value_legend(&values) {
        print!("\n{legend}");
    }
    let _ = std::io::stdout().flush();

    let mut stderr = std::io::stderr();
    for diagnostic in &report.failures {
        reporter.print_error(&Error::single(diagnostic.clone()), &mut stderr);
    }
    if report.failures.is_empty() {
        ExitCode::Success
    } else {
        ExitCode::Failure
    }
}

/// `ls`の出力。
///
/// 0件でもheaderを表示する。
fn print_listing(catalog: &Catalog, listing: &ls::Listing) {
    let reporter = Reporter::new(catalog);
    let projects: Vec<Vec<String>> = listing
        .projects
        .iter()
        .map(|row| {
            vec![
                row.project.clone(),
                row.sandbox.clone(),
                row.state.as_str().to_string(),
            ]
        })
        .collect();
    println!("{}", text_or_report(catalog, "ls-projects-section"));
    print!(
        "{}",
        reporter.render_value_table(
            &["column-project", "column-sandbox", "column-state"],
            &projects,
        )
    );

    if !listing.unmanaged.is_empty() {
        let unmanaged: Vec<Vec<String>> = listing
            .unmanaged
            .iter()
            .map(|row| {
                vec![
                    row.sandbox.clone(),
                    row.state.clone(),
                    row.workspace.clone(),
                ]
            })
            .collect();
        println!("\n{}", text_or_report(catalog, "ls-unmanaged-section"));
        print!(
            "{}",
            reporter.render_value_table(
                &["column-sandbox", "column-state", "column-workspace"],
                &unmanaged,
            )
        );
    }

    let values: Vec<(&str, &str)> = listing
        .projects
        .iter()
        .map(|row| (row.state.as_str(), row.state.legend_id()))
        .collect();
    if let Some(legend) = reporter.render_value_legend(&values) {
        print!("\n{legend}");
    }
    let _ = std::io::stdout().flush();
}

/// `sync-files`の成功出力。
fn print_sync_output(catalog: &Catalog, output: &workflow::sync_files::SyncOutput) {
    let reporter = Reporter::new(catalog);
    println!(
        "{}",
        format_or_report(
            catalog,
            &msg!(
                "sync-files-done",
                count = output.files.len(),
                project = output.project,
                sandbox = output.sandbox
            )
        )
    );

    let files: Vec<Vec<String>> = output
        .files
        .iter()
        .map(|file| {
            vec![
                paths::display(&file.source),
                file.destination.clone(),
                file.placement.as_str().to_string(),
            ]
        })
        .collect();
    if !files.is_empty() {
        print!(
            "\n{}",
            reporter.render_value_table(
                &["column-file", "column-destination", "column-result"],
                &files,
            )
        );
        println!("{}", text_or_report(catalog, "files-secret-hint"));
        let values: Vec<(&str, &str)> = output
            .files
            .iter()
            .map(|file| placement_legend(file.placement))
            .collect();
        if let Some(legend) = reporter.render_value_legend(&values) {
            print!("\n{legend}");
        }
    }
    let _ = std::io::stdout().flush();
}

/// 翻訳しない状態値と、その凡例。
fn sandbox_state(state: SandboxState) -> &'static str {
    match state {
        SandboxState::Running => "running",
        SandboxState::Stopped => "stopped",
    }
}

fn sandbox_state_legend(state: SandboxState) -> &'static str {
    match state {
        SandboxState::Running => "legend-sandbox-running",
        SandboxState::Stopped => "legend-sandbox-stopped",
    }
}

fn mode_legend(mode: CreationMode) -> (&'static str, &'static str) {
    match mode {
        CreationMode::Attached => ("attached", "legend-attached"),
        CreationMode::Detached => ("detached", "legend-detached"),
    }
}

fn placement_legend(placement: Placement) -> (&'static str, &'static str) {
    match placement {
        Placement::Placed => ("placed", "legend-placed"),
        Placement::Unchanged => ("unchanged", "legend-unchanged"),
    }
}

/// 案件を対象とするcommandが必要とするconfigとcatalog。
fn require_config(
    location: &ConfigLocation,
    lang_option: Option<Locale>,
) -> Result<(Box<config::GlobalConfig>, Catalog)> {
    match config::load(location)? {
        ConfigState::Valid { config, .. } => {
            let catalog = Catalog::new(lang_option.unwrap_or(config.language));
            Ok((config, catalog))
        }
        ConfigState::Missing => Err(Error::single(
            Diagnostic::new(
                ErrorId::ConfigMissing,
                msg!(
                    "error-config-missing",
                    path = paths::display(&location.config_file())
                ),
            )
            .remediation(msg!("remediation-run-init")),
        )),
    }
}

/// commandが使うcatalogを、configのvalidationを経て決める。
///
/// `status --global`はconfigがなくても診断を続けるため、不在を許容する。
fn effective_catalog(
    location: &ConfigLocation,
    lang_option: Option<Locale>,
    allow_missing_config: bool,
) -> Result<Catalog> {
    match config::load(location)? {
        ConfigState::Valid { config, .. } => {
            Ok(Catalog::new(lang_option.unwrap_or(config.language)))
        }
        ConfigState::Missing if allow_missing_config => Ok(Catalog::new(
            lang_option.or_else(shell_locale).unwrap_or(Locale::En),
        )),
        ConfigState::Missing => Err(Error::single(
            Diagnostic::new(
                ErrorId::ConfigMissing,
                msg!(
                    "error-config-missing",
                    path = paths::display(&location.config_file())
                ),
            )
            .remediation(msg!("remediation-run-init")),
        )),
    }
}

fn report(catalog: &Catalog, error: &Error) {
    // Ctrl-CとEscは何も変更していないため、追加の説明を出さない。
    if matches!(error, Error::Canceled) {
        return;
    }
    let reporter = Reporter::new(catalog);
    reporter.print_error(error, &mut std::io::stderr());
}

fn format_or_report(catalog: &Catalog, message: &error::Msg) -> String {
    catalog
        .format(message)
        .unwrap_or_else(|failure| failure.to_string())
}

fn text_or_report(catalog: &Catalog, id: &str) -> String {
    catalog
        .text(id)
        .unwrap_or_else(|failure| failure.to_string())
}
