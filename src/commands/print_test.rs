//! command printerが並べる構造。
//!
//! 翻訳文の微修正でtestが壊れないよう、文言ではなくblockの順序と役割を確かめる。
//! どう描かれるかは`src/design`のinvariant testが持ち、ここは何を並べるかだけを見る。

use crate::testing::outcome::{Checked, Required, Unmet};

use std::path::PathBuf;

use crate::compatibility::SandboxState;
use crate::design::{Block, SectionBody};
use crate::design::{Document, Inline, VisualState};
use crate::diagnostics::{Diagnostic, ErrorId, Msg};
use crate::i18n::Locale;
use crate::metadata::CreationMode;
use crate::msg;
use crate::support::files::{PlacedFile, Placement};
use crate::support::inventory::{Observed, ProjectState};
use crate::support::status::{Row, StatusValue};
use crate::testing::plain;

/// blockの並びを役割の名前で表す。
fn shape(document: &Document) -> Vec<&'static str> {
    document
        .blocks()
        .iter()
        .map(|block| match block {
            Block::Progress(_) => "progress",
            Block::Summary(_) => "summary",
            Block::Section(section) => match section.body {
                SectionBody::Fields(_) => "fields",
                SectionBody::Table(_) => "table",
                SectionBody::Lines(_) => "lines",
                SectionBody::Legend(_) => "legend",
                SectionBody::Empty(_) => "empty",
            },
            Block::Guidance(_) => "guidance",
            Block::Warning(_) => "warning",
            Block::Note(_) => "note",
            Block::Command(_) => "command",
            Block::Diagnostic(_) => "diagnostic",
            Block::Verbatim(_) => "verbatim",
        })
        .collect()
}

/// documentに含まれるcommand行。
fn commands(document: &Document) -> Vec<&str> {
    document
        .blocks()
        .iter()
        .filter_map(|block| match block {
            Block::Command(command) => Some(command.as_str()),
            _ => None,
        })
        .collect()
}

fn add_output() -> super::add::AddOutput {
    super::add::AddOutput {
        project: "owner/repo".to_string(),
        sandbox: "owner-repo".to_string(),
        mode: CreationMode::Attached,
        start_ref: Some("main".to_string()),
        requested_worktrees: 1,
        host_clone: PathBuf::from("/tmp/owner-repo"),
        already_registered: false,
        warnings: Vec::new(),
    }
}

#[test]
fn add_separates_each_next_step_from_the_command_it_asks_for() {
    let document = super::add::print::document(&add_output());
    assert_eq!(
        shape(&document),
        vec![
            "summary", "fields", "guidance", "command", "guidance", "command", "guidance",
            "command"
        ]
    );
    // 案件IDを打ち直させないため、次のcommandはそのままcopyできる形で並べる。
    assert_eq!(
        commands(&document)[1..],
        ["sbxm prepare owner/repo", "sbxm open owner/repo"]
    );
}

#[test]
fn add_reports_a_repeat_run_without_pretending_something_changed() -> Checked {
    let mut output = add_output();
    output.already_registered = true;
    let drawn = plain(&super::add::print::document(&output), Locale::En)?;
    assert!(
        drawn.starts_with("\u{2713} owner/repo is already managed"),
        "{drawn}"
    );
    Ok(())
}

fn placed() -> Vec<PlacedFile> {
    vec![PlacedFile {
        source: PathBuf::from("/home/user/.gitconfig"),
        destination: ".gitconfig".to_string(),
        placement: Placement::Placed,
    }]
}

fn prepare_output() -> super::prepare::PrepareOutput {
    super::prepare::PrepareOutput {
        project: "owner/repo".to_string(),
        sandbox: "owner-repo".to_string(),
        mode: CreationMode::Attached,
        start_ref: "main".to_string(),
        sandbox_state: SandboxState::Running,
        worktrees: vec![super::prepare::WorktreeRow {
            path: "workspace".to_string(),
            created_from: "main".to_string(),
            head: Some("a1b2c3d".to_string()),
            mode: CreationMode::Attached,
        }],
        files: placed(),
        notes: Vec::new(),
        already_built: false,
        warnings: Vec::new(),
    }
}

#[test]
fn prepare_keeps_the_security_note_off_the_end_of_the_table() {
    let document = super::prepare::print::document(&prepare_output(), Locale::En);
    assert_eq!(
        shape(&document),
        vec!["summary", "fields", "table", "table", "note"]
    );
}

#[test]
fn prepare_adds_a_legend_only_where_the_values_are_not_the_source_language() {
    let english = super::prepare::print::document(&prepare_output(), Locale::En);
    assert!(!shape(&english).contains(&"legend"));

    let japanese = super::prepare::print::document(&prepare_output(), Locale::Ja);
    assert_eq!(shape(&japanese).last(), Some(&"legend"));
}

#[test]
fn prepare_leaves_out_a_table_it_has_no_rows_for() {
    let mut output = prepare_output();
    output.worktrees.clear();
    output.files.clear();
    assert_eq!(
        shape(&super::prepare::print::document(&output, Locale::En)),
        vec!["summary", "fields"]
    );
}

#[test]
fn a_tool_note_puts_what_the_user_must_run_on_its_own_line() -> Checked {
    let notes = vec![crate::support::tools::Note {
        heading: msg!("add-mise-heading"),
        items: vec!["/workspace/mise.toml".to_string()],
        hint: msg!("add-mise-hint"),
        commands: vec![
            crate::design::CommandLine::new("mise trust").required_because("one line")?,
            crate::design::CommandLine::new("mise install").required_because("one line")?,
        ],
    }];
    let document = super::prepare::print::notes(&notes);
    assert_eq!(
        shape(&document),
        vec!["lines", "guidance", "command", "command"]
    );
    assert_eq!(commands(&document), vec!["mise trust", "mise install"]);
    Ok(())
}

fn apply_output(worktrees: Option<u32>, files: Vec<PlacedFile>) -> super::apply::ApplyOutput {
    super::apply::ApplyOutput {
        project: "owner/repo".to_string(),
        sandbox: "owner-repo".to_string(),
        files,
        worktrees,
        notes: Vec::new(),
    }
}

#[test]
fn apply_reports_only_the_scope_it_was_asked_for() {
    // worktreeだけを適用した実行で、fileの結果を0件として報告しない。
    let worktrees_only =
        super::apply::print::document(&apply_output(Some(2), Vec::new()), Locale::En);
    assert_eq!(shape(&worktrees_only), vec!["summary"]);

    let files_only = super::apply::print::document(&apply_output(None, placed()), Locale::En);
    assert_eq!(shape(&files_only), vec!["summary", "table", "note"]);

    let both = super::apply::print::document(&apply_output(Some(2), placed()), Locale::En);
    assert_eq!(
        shape(&both),
        vec!["summary", "summary", "table", "note"],
        "each result gets its own line"
    );
}

fn listing() -> super::ls::Listing {
    super::ls::Listing {
        projects: vec![super::ls::ProjectRow {
            project: "owner/repo".to_string(),
            root: "/home/user/Projects/repo.project".to_string(),
            sandbox: "owner-repo".to_string(),
            observed: Observed::Registered(ProjectState::Running),
        }],
        unmanaged: Vec::new(),
        settled: true,
    }
}

#[test]
fn ls_shows_the_unmanaged_section_only_when_there_is_one() {
    assert_eq!(
        shape(&super::ls::print::document(&listing(), Locale::En)),
        vec!["table"]
    );

    let mut with_unmanaged = listing();
    with_unmanaged.unmanaged.push(super::ls::UnmanagedRow {
        sandbox: "other".to_string(),
        state: "running".to_string(),
        workspace: "/tmp/other".to_string(),
    });
    assert_eq!(
        shape(&super::ls::print::document(&with_unmanaged, Locale::En)),
        vec!["table", "table"]
    );
}

#[test]
fn ls_says_so_when_there_is_nothing_to_list() {
    // 対象ゼロであること自体が答えであるため、sectionごと省かない。
    let empty = super::ls::Listing {
        projects: Vec::new(),
        unmanaged: Vec::new(),
        settled: true,
    };
    assert_eq!(
        shape(&super::ls::print::document(&empty, Locale::En)),
        vec!["empty"]
    );
}

fn global_status() -> super::status::global::GlobalStatus {
    super::status::global::GlobalStatus {
        rows: vec![
            Row {
                item: "status-item-config",
                status: StatusValue::Ready,
            },
            Row {
                item: "status-item-docker",
                status: StatusValue::Error,
            },
        ],
        warnings: Vec::new(),
        diagnostics: Vec::new(),
    }
}

#[test]
fn global_status_is_a_table_and_nothing_else() {
    // 表そのものが結論であるためsummaryを足さない。
    assert_eq!(
        shape(&super::status::print::global_document(
            &global_status(),
            Locale::En
        )),
        vec!["table"]
    );
}

#[test]
fn a_status_value_carries_the_state_its_context_gives_it() -> Checked {
    let document = super::status::print::global_document(&global_status(), Locale::En);
    let Block::Section(section) = &document.blocks()[0] else {
        return Err(Unmet::new("a table".to_string()));
    };
    let SectionBody::Table(table) = &section.body else {
        return Err(Unmet::new("a table".to_string()));
    };
    assert_eq!(
        table.rows()[0][1],
        Inline::state("ready", VisualState::Positive).into()
    );
    assert_eq!(
        table.rows()[1][1],
        Inline::state("error", VisualState::Negative).into()
    );
    Ok(())
}

fn project_status(
    worktrees: Vec<super::status::project::WorktreeRow>,
) -> super::status::project::ProjectStatus {
    super::status::project::ProjectStatus {
        project: "owner/repo".to_string(),
        items: vec![super::status::project::Item {
            item: "status-item-metadata",
            value: super::status::project::Value::Ready,
        }],
        worktrees,
        diagnostics: vec![Diagnostic::new(
            ErrorId::GlobalScopeUnobservable,
            msg!("error-global-scope-unobservable"),
        )],
    }
}

#[test]
fn project_status_says_that_no_worktree_was_observed() {
    assert_eq!(
        shape(&super::status::print::project_document(
            &project_status(Vec::new()),
            Locale::En
        )),
        vec!["fields", "empty"]
    );
}

#[test]
fn project_status_lists_the_worktrees_it_did_observe() {
    let worktrees = vec![super::status::project::WorktreeRow {
        path: "workspace".to_string(),
        kind: "managed",
        mode: super::status::project::Value::Attached,
        state: super::status::project::Value::Clean,
    }];
    assert_eq!(
        shape(&super::status::print::project_document(
            &project_status(worktrees),
            Locale::En
        )),
        vec!["fields", "table"]
    );
}

#[test]
fn a_diagnostic_never_reaches_the_result_document() {
    // stdoutは正常結果だけを持つ。診断はstderrのblockとして別に出す。
    let document = super::status::print::project_document(&project_status(Vec::new()), Locale::En);
    assert!(!shape(&document).contains(&"diagnostic"));
}

fn stop_report(result: super::stop::StopResult) -> super::stop::StopReport {
    super::stop::StopReport {
        outcomes: vec![super::stop::StopOutcome {
            project: "owner/repo".to_string(),
            sandbox: "owner-repo".to_string(),
            result,
        }],
        failures: Vec::new(),
    }
}

#[test]
fn stopping_a_sandbox_is_a_success_even_though_the_word_is_stopped() -> Checked {
    let document =
        super::stop::print::document(&stop_report(super::stop::StopResult::Stopped), Locale::En);
    let Block::Section(section) = &document.blocks()[0] else {
        return Err(Unmet::new("a table".to_string()));
    };
    let SectionBody::Table(table) = &section.body else {
        return Err(Unmet::new("a table".to_string()));
    };
    assert_eq!(
        table.rows()[0][2],
        Inline::state("stopped", VisualState::Positive).into()
    );
    Ok(())
}

fn destroy_plan(force: bool) -> super::destroy::run::DestroyPlan {
    super::destroy::run::DestroyPlan {
        project: "owner/repo".to_string(),
        sandbox: "owner-repo".to_string(),
        state: ProjectState::Running,
        force,
        worktrees: Vec::new(),
        removes: vec![super::destroy::run::Target::Described(msg!(
            "destroy-target-sandbox",
            sandbox = "owner-repo"
        ))],
        keeps: vec![super::destroy::run::Target::Path(
            "/tmp/owner-repo".to_string(),
        )],
        re_register: "sbxm add owner/repo".to_string(),
    }
}

#[test]
fn the_deletion_plan_separates_what_goes_from_what_stays() {
    let document = super::destroy::print::plan_document(&destroy_plan(false), Locale::En);
    assert_eq!(
        shape(&document),
        vec!["fields", "lines", "lines", "guidance", "command"]
    );
    assert_eq!(commands(&document), vec!["sbxm add owner/repo"]);
}

#[test]
fn the_deletion_plan_never_paints_anything_green() -> Checked {
    // 破壊操作の確認画面で成功色を使わない。消えるものと残るものを落ち着いて比べる。
    let drawn = plain(
        &super::destroy::print::plan_document(&destroy_plan(false), Locale::En),
        Locale::En,
    )?;
    assert!(!drawn.contains('\u{1b}'), "the plan is plain here: {drawn}");

    let mut buffer: Vec<u8> = Vec::new();
    {
        let mut renderer = crate::design::renderer::Renderer::new(
            &mut buffer,
            crate::design::policy::StreamPolicy::colored(),
        );
        renderer.write(
            &crate::i18n::Catalog::new(Locale::En),
            &super::destroy::print::plan_document(&destroy_plan(false), Locale::En),
        );
    }
    let colored = String::from_utf8(buffer).required_because("UTF-8")?;
    assert!(
        !colored.contains("\u{1b}[32m"),
        "green belongs to success, not to a deletion plan: {colored:?}"
    );
    Ok(())
}

#[test]
fn force_mode_is_a_warning_rather_than_part_of_the_plan() {
    let notice: Msg = super::destroy::print::force_notice().description;
    assert_eq!(notice.id, "destroy-force-notice");
    assert!(
        !shape(&super::destroy::print::plan_document(
            &destroy_plan(true),
            Locale::En
        ))
        .contains(&"warning")
    );
}

#[test]
fn the_result_of_a_deletion_says_how_to_get_the_project_back() {
    let outcome = super::destroy::run::DestroyOutcome {
        project: "owner/repo".to_string(),
        re_register: "sbxm add owner/repo".to_string(),
        warnings: Vec::new(),
    };
    let document = super::destroy::print::outcome_document(&outcome);
    assert_eq!(shape(&document), vec!["summary", "guidance", "command"]);
}
