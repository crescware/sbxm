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
use crate::support::inventory::{Observed, ProjectState, WorkspaceState};
use crate::support::protection::{ConfirmableLoss, Kind, Mode, Remote, WorktreeReport};
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
            Block::Warning { .. } => "warning",
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
        already_built: false,
        warnings: Vec::new(),
    }
}

#[test]
fn prepare_keeps_the_security_note_off_the_end_of_the_table() {
    let document = super::prepare::print::document(&prepare_output(), Locale::En);
    assert_eq!(
        shape(&document),
        vec![
            "summary", "fields", "table", "table", "note", "guidance", "command"
        ]
    );
}

#[test]
fn prepare_closes_with_the_command_that_opens_what_it_just_built() {
    // 構築の次はSSHで入ることであり、`add`もその順で案内する。ここで案内しないと、
    // 利用者は`add`の案内を遡って読み直すことになる。
    let document = super::prepare::print::document(&prepare_output(), Locale::En);
    assert_eq!(commands(&document), vec!["sbxm open owner/repo"]);
    assert_eq!(shape(&document).last(), Some(&"command"));
}

#[test]
fn prepare_adds_a_legend_only_where_the_values_are_not_the_source_language() -> Checked {
    let english = super::prepare::print::document(&prepare_output(), Locale::En);
    assert!(!shape(&english).contains(&"legend"));

    let japanese = shape(&super::prepare::print::document(
        &prepare_output(),
        Locale::Ja,
    ));
    let legend = japanese
        .iter()
        .position(|block| *block == "legend")
        .required_because("the legend describes the values above it")?;
    assert_eq!(
        japanese[legend + 1..],
        ["guidance", "command"],
        "the next step stays last: {japanese:?}"
    );
    Ok(())
}

#[test]
fn prepare_leaves_out_a_table_it_has_no_rows_for() {
    let mut output = prepare_output();
    output.worktrees.clear();
    output.files.clear();
    assert_eq!(
        shape(&super::prepare::print::document(&output, Locale::En)),
        vec!["summary", "fields", "guidance", "command"]
    );
}

fn apply_output(worktrees: Option<u32>, files: Vec<PlacedFile>) -> super::apply::ApplyOutput {
    super::apply::ApplyOutput {
        project: "owner/repo".to_string(),
        sandbox: "owner-repo".to_string(),
        files,
        worktrees,
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
            workspace: WorkspaceState::Ready,
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
        disk: crate::support::disk::DiskObservation::NotObservedMismatch,
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
        vec!["fields", "empty", "empty"]
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
        vec!["fields", "table", "empty"]
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
        confirmable_losses: Vec::new(),
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

/// 通常modeで観測した2件のworktreeを持つ計画。
///
/// 状態値の組み合わせをすべて1度ずつ通し、列に置く値が対応する種別から来ることを見る。
fn destroy_plan_with_worktrees() -> super::destroy::run::DestroyPlan {
    super::destroy::run::DestroyPlan {
        worktrees: vec![
            WorktreeReport {
                relative: "repo.tree-0".to_string(),
                kind: Kind::Managed,
                mode: Mode::Attached,
                head: "a1b2c3d".to_string(),
                branch: Some("main".to_string()),
                remote: Remote::Pushed,
            },
            WorktreeReport {
                relative: "repo.scratch".to_string(),
                kind: Kind::Unmanaged,
                mode: Mode::Detached,
                head: "d4e5f6a".to_string(),
                branch: None,
                remote: Remote::Reachable,
            },
        ],
        ..destroy_plan(false)
    }
}

/// 層Bの確認対象を1 variantずつ並べた一覧。
///
/// 削除計画は観測した損失を1件も落とさずに見せる。variantを足したときに説明のない行が
/// 出ないよう、全variantをここへ置いて表示まで通す。
fn every_confirmable_loss() -> Vec<ConfirmableLoss> {
    vec![
        ConfirmableLoss::IgnoredPaths {
            worktree: "repo.tree-0".to_string(),
            paths: vec!["node_modules/".to_string(), "target/".to_string()],
        },
        ConfirmableLoss::LocalRef {
            reference: "refs/stash".to_string(),
        },
        ConfirmableLoss::BranchUpstream {
            branch: "topic".to_string(),
            upstream: "origin/topic".to_string(),
        },
        ConfirmableLoss::Tag {
            name: "v1".to_string(),
        },
        ConfirmableLoss::AdditionalRemote {
            name: "fork".to_string(),
        },
        ConfirmableLoss::ReflogOnlyCommits { count: 3 },
        ConfirmableLoss::UnmanagedWorktree {
            worktree: "repo.scratch".to_string(),
        },
        ConfirmableLoss::SandboxWritableLayer,
    ]
}

/// documentが持つ`index`番目のsectionの行。
fn lines_at(document: &Document, index: usize) -> Checked<&Vec<crate::design::Cell>> {
    let Some(Block::Section(section)) = document.blocks().get(index) else {
        return Err(Unmet::new("a section".to_string()));
    };
    let SectionBody::Lines(lines) = &section.body else {
        return Err(Unmet::new("lines".to_string()));
    };
    Ok(lines)
}

/// 行が持つmessage IDの並び。
fn line_ids(lines: &[crate::design::Cell]) -> Vec<&'static str> {
    lines
        .iter()
        .map(|cell| match cell {
            crate::design::Cell::Label(message) => message.id,
            crate::design::Cell::Value(_) => "value",
        })
        .collect()
}

#[test]
fn the_deletion_plan_explains_every_kind_of_loss_it_observed() -> Checked {
    // 層Bの損失は、確認を求める前に1件残らず見せる。説明を持たないvariantがあると、
    // 利用者は何を失うのかを知らないまま名前を入力することになる。
    let plan = super::destroy::run::DestroyPlan {
        confirmable_losses: every_confirmable_loss(),
        ..destroy_plan_with_worktrees()
    };
    let document = super::destroy::print::plan_document(&plan, Locale::En);
    assert_eq!(
        shape(&document),
        vec![
            "fields", "table", "lines", "lines", "lines", "guidance", "command"
        ]
    );
    assert_eq!(
        line_ids(lines_at(&document, 2)?),
        vec![
            "confirmable-loss-ignored-paths",
            "confirmable-loss-local-ref",
            "confirmable-loss-branch-upstream",
            "confirmable-loss-tag",
            "confirmable-loss-additional-remote",
            "confirmable-loss-reflog-only-commits",
            "confirmable-loss-unmanaged-worktree",
            "confirmable-loss-sandbox-writable-layer",
        ]
    );
    Ok(())
}

#[test]
fn the_rebuild_plan_shows_both_generations_and_what_the_rebuild_loses() -> Checked {
    // rebuildもdestroyと同じ層Bの一覧を見せる。世代のhashは、どちらへ動くのかが
    // 読めるよう現在と適用先を並べる。
    let plan = super::rebuild::run::RebuildPlan {
        project: "owner/repo".to_string(),
        sandbox: "sbxm-owner-repo-0123456789ab".to_string(),
        current_generation: "a".repeat(64),
        target_generation: "b".repeat(64),
        confirmable_losses: every_confirmable_loss(),
    };
    let document = super::rebuild::print::plan_document(&plan);
    assert_eq!(shape(&document), vec!["fields", "lines"]);

    let Some(Block::Section(section)) = document.blocks().first() else {
        return Err(Unmet::new("a section".to_string()));
    };
    let SectionBody::Fields(fields) = &section.body else {
        return Err(Unmet::new("fields".to_string()));
    };
    assert_eq!(
        fields
            .iter()
            .map(|field| field.label.id)
            .collect::<Vec<&str>>(),
        vec![
            "add-field-project",
            "add-field-sandbox",
            "rebuild-plan-current-generation",
            "rebuild-plan-target-generation",
        ]
    );
    assert_eq!(
        line_ids(lines_at(&document, 1)?).len(),
        every_confirmable_loss().len(),
        "the rebuild plan drops none of the losses it observed"
    );
    Ok(())
}

#[test]
fn a_plan_without_a_single_loss_shows_no_empty_section() {
    // 失うものが1件も無いことと、見出しだけがあることは別である。
    let document = super::rebuild::print::plan_document(&super::rebuild::run::RebuildPlan {
        project: "owner/repo".to_string(),
        sandbox: "sbxm-owner-repo-0123456789ab".to_string(),
        current_generation: "a".repeat(64),
        target_generation: "a".repeat(64),
        confirmable_losses: Vec::new(),
    });
    assert_eq!(shape(&document), vec!["fields"]);
}

/// documentが持つ`index`番目のtable。
fn table_at(document: &Document, index: usize) -> Checked<&crate::design::Table> {
    let Some(Block::Section(section)) = document.blocks().get(index) else {
        return Err(Unmet::new("a section".to_string()));
    };
    let SectionBody::Table(table) = &section.body else {
        return Err(Unmet::new("a table".to_string()));
    };
    Ok(table)
}

#[test]
fn the_deletion_plan_shows_what_each_worktree_holds_before_it_goes() -> Checked {
    let document = super::destroy::print::plan_document(&destroy_plan_with_worktrees(), Locale::En);
    assert_eq!(
        shape(&document),
        vec!["fields", "table", "lines", "lines", "guidance", "command"]
    );

    let rows = table_at(&document, 1)?.rows();
    assert_eq!(
        rows[0],
        vec![
            Inline::path("repo.tree-0").into(),
            Inline::text("managed").into(),
            Inline::text("attached").into(),
            Inline::text("main").into(),
            Inline::text("a1b2c3d").into(),
            Inline::text("pushed").into(),
        ]
    );
    // detachedなworktreeにbranchはない。列を詰めず、値がないことを見せる。
    assert_eq!(
        rows[1],
        vec![
            Inline::path("repo.scratch").into(),
            Inline::text("unmanaged").into(),
            Inline::text("detached").into(),
            Inline::text("-").into(),
            Inline::text("d4e5f6a").into(),
            Inline::text("reachable").into(),
        ]
    );
    Ok(())
}

#[test]
fn every_worktree_value_the_plan_showed_gets_an_explanation() -> Checked {
    // 状態値は翻訳しない。正本locale以外では、出現した値だけを凡例で説明する。
    let japanese = super::destroy::print::plan_document(&destroy_plan_with_worktrees(), Locale::Ja);
    let Some(Block::Section(section)) = japanese.blocks().last() else {
        return Err(Unmet::new("a section".to_string()));
    };
    let SectionBody::Legend(entries) = &section.body else {
        return Err(Unmet::new("a legend".to_string()));
    };
    let explained: Vec<(&str, &str)> = entries
        .iter()
        .map(|entry| (entry.value.as_str(), entry.description.id))
        .collect();
    assert_eq!(
        explained,
        vec![
            ("attached", "legend-attached"),
            ("detached", "legend-detached"),
            ("managed", "legend-managed"),
            ("pushed", "legend-pushed"),
            ("reachable", "legend-reachable"),
            ("running", "legend-sandbox-running"),
            ("unmanaged", "legend-unmanaged"),
        ]
    );

    // 正本localeでは値がそのまま読めるため、凡例そのものを置かない。
    let english = super::destroy::print::plan_document(&destroy_plan_with_worktrees(), Locale::En);
    assert!(!shape(&english).contains(&"legend"));
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
