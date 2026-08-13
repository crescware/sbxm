use crate::commands::status::project::Value as ProjectValue;
use crate::commands::stop::StopResult;
use crate::compatibility::SandboxState;
use crate::design::{Inline, VisualState};
use crate::i18n::Locale;
use crate::metadata::CreationMode;
use crate::support::files::Placement;
use crate::support::inventory::{Observed, ProjectState, WorkspaceState};
use crate::support::status::StatusValue;

use crate::testing::outcome::{Checked, Required};

use super::*;

/// project scopeが表に出す状態値のすべて。
const PROJECT_VALUES: [ProjectValue; 16] = [
    ProjectValue::Ready,
    ProjectValue::Missing,
    ProjectValue::Mismatch,
    ProjectValue::NotObserved,
    ProjectValue::Changed,
    ProjectValue::Running,
    ProjectValue::Stopped,
    ProjectValue::NotCreated,
    ProjectValue::Clean,
    ProjectValue::Dirty,
    ProjectValue::Attached,
    ProjectValue::Detached,
    ProjectValue::NotExposed,
    ProjectValue::Exposed,
    ProjectValue::NotApplicable,
    ProjectValue::NotObservedStopped,
];

#[test]
fn the_same_word_means_different_things_in_different_commands() {
    // `stopped`は稼働要件のstatusでは注意、停止commandの結果では成功である。
    let required = global_status(StatusValue::Stopped);
    let finished = stop_result(StopResult::Stopped);
    assert_eq!(required.as_str(), finished.as_str());
    assert_eq!(
        required,
        Inline::state("stopped", VisualState::Attention),
        "a service that should be running is not positive"
    );
    assert_eq!(finished, Inline::state("stopped", VisualState::Positive));
}

#[test]
fn an_exposed_host_agent_is_a_failure_and_not_a_warning() {
    assert_eq!(
        project_status(ProjectValue::Exposed),
        Inline::state("exposed", VisualState::Negative)
    );
    assert_eq!(
        project_status(ProjectValue::NotExposed),
        Inline::state("not-exposed", VisualState::Positive)
    );
}

#[test]
fn a_configuration_choice_carries_no_judgement() {
    for value in [ProjectValue::Attached, ProjectValue::Detached] {
        assert_eq!(
            project_status(value),
            Inline::state(value.as_str(), VisualState::Neutral)
        );
    }
    assert_eq!(
        creation_mode(CreationMode::Attached),
        Inline::state("attached", VisualState::Neutral)
    );
}

#[test]
fn an_entry_that_disagrees_with_its_artifacts_is_a_failure_and_not_a_warning() {
    // 続きを実行すれば済む中断は注意にとどめ、registryと成果物の食い違いは失敗として示す。
    assert_eq!(
        observed(&Observed::Incomplete),
        Inline::state("incomplete", VisualState::Attention)
    );
    for value in [Observed::Missing, Observed::Inconsistent] {
        assert_eq!(
            observed(&value),
            Inline::state(value.as_str(), VisualState::Negative),
            "{} is not a warning",
            value.as_str()
        );
    }
    // 登録が済んだ案件だけは、Sandboxの状態そのものを示す。
    assert_eq!(
        observed(&Observed::Registered(ProjectState::Running)),
        project_state(ProjectState::Running)
    );
}

#[test]
fn every_project_value_has_a_state_that_does_not_panic() {
    for value in PROJECT_VALUES {
        assert_eq!(project_status(value).as_str(), value.as_str());
    }
}

#[test]
fn every_project_value_is_explained_by_a_legend_the_catalog_can_render() -> Checked {
    // 状態値は翻訳しない。翻訳先で読めるのは凡例だけであり、説明の無い値は残せない。
    let catalog = crate::i18n::Catalog::new(Locale::Ja);
    let mut explained = std::collections::BTreeMap::new();
    for value in PROJECT_VALUES {
        let description = catalog
            .text(value.legend_id())
            .required_because(&format!("{} has a legend", value.as_str()))?;
        assert!(
            !description.is_empty(),
            "{} has an empty legend",
            value.as_str()
        );
        if let Some(previous) = explained.insert(value.as_str(), value.legend_id()) {
            assert_eq!(
                previous,
                value.legend_id(),
                "{} is explained two different ways",
                value.as_str()
            );
        }
    }
    Ok(())
}

#[test]
fn the_source_locale_needs_no_legend() {
    let mut legend = Legend::new(Locale::SOURCE);
    legend.add("ready", "legend-ready");
    assert!(legend.entries().is_empty());
}

#[test]
fn a_translated_locale_describes_only_the_values_that_appeared() {
    let mut legend = Legend::new(Locale::Ja);
    legend.add("ready", "legend-ready");
    legend.add("missing", "legend-missing");
    let entries = legend.entries();
    let values: Vec<&str> = entries.iter().map(|entry| entry.value.as_str()).collect();
    assert_eq!(values, vec!["missing", "ready"], "sorted and deduplicated");
}

#[test]
fn the_same_value_is_listed_once() {
    let mut legend = Legend::new(Locale::Ja);
    legend.add("running", "legend-sandbox-running");
    legend.add("running", "legend-sandbox-running");
    assert_eq!(legend.entries().len(), 1);
}

#[test]
fn a_sandbox_that_exists_but_is_not_running_is_not_a_finished_state() {
    // 構築の結果として見せるstoppedは、そこから先へ進むために起動が要る状態である。
    // 停止commandの完了結果と同じ語だが、判断は逆になる。
    assert_eq!(
        sandbox_state(SandboxState::Running),
        Inline::state("running", VisualState::Positive)
    );
    assert_eq!(
        sandbox_state(SandboxState::Stopped),
        Inline::state("stopped", VisualState::Attention)
    );
    assert_eq!(
        sandbox_state(SandboxState::Stopped).as_str(),
        stop_result(StopResult::Stopped).as_str(),
        "the same word carries the opposite judgement"
    );
}

#[test]
fn a_project_whose_sandbox_is_not_running_is_not_settled() {
    // 一覧のrunningだけが手を入れずに使える状態である。停止中も未作成も、
    // 作業を始めるには何かをしなければならない。
    assert_eq!(
        project_state(ProjectState::Running),
        Inline::state("running", VisualState::Positive)
    );
    for state in [ProjectState::Stopped, ProjectState::NotCreated] {
        assert_eq!(
            project_state(state),
            Inline::state(state.as_str(), VisualState::Attention),
            "{} is not a settled state",
            state.as_str()
        );
    }
}

#[test]
fn a_workspace_that_is_gone_is_a_missing_start_up_condition_and_not_a_lost_artifact() {
    // 中立workspaceは空のmount点である。消えていても案件の成果物は失われていないため、
    // registryとの食い違いと同じ失敗としては示さない。
    assert_eq!(
        workspace_state(WorkspaceState::Ready),
        Inline::state("ready", VisualState::Positive)
    );
    for state in [WorkspaceState::Missing, WorkspaceState::NotObserved] {
        assert_eq!(
            workspace_state(state),
            Inline::state(state.as_str(), VisualState::Attention),
            "{} needs attention rather than reading as a failure",
            state.as_str()
        );
    }
    // 見に行く対象が無いことは、良し悪しではない。
    assert_eq!(
        workspace_state(WorkspaceState::NotApplicable),
        Inline::state("not-applicable", VisualState::Neutral)
    );
}

#[test]
fn every_workspace_state_is_explained_by_a_legend_the_catalog_can_render() -> Checked {
    // `ls`のWORKSPACE列は`status`の`status-item-workspace`と同じ語彙で示す。翻訳先で
    // 読めるのは凡例だけであり、説明の無い値は残せない。
    let catalog = crate::i18n::Catalog::new(Locale::Ja);
    for state in [
        WorkspaceState::Ready,
        WorkspaceState::Missing,
        WorkspaceState::NotObserved,
        WorkspaceState::NotApplicable,
    ] {
        let description = catalog
            .text(state.legend_id())
            .required_because(&format!("{} has a legend", state.as_str()))?;
        assert!(
            !description.is_empty(),
            "{} has an empty legend",
            state.as_str()
        );
    }
    Ok(())
}

#[test]
fn a_file_the_sandbox_already_held_is_not_reported_as_a_change() {
    // 書き込んだ配置だけを成果として示す。同じ内容だった配置は、良し悪しの判断を
    // 持たない事実である。
    assert_eq!(
        placement(Placement::Placed),
        Inline::state("placed", VisualState::Positive)
    );
    assert_eq!(
        placement(Placement::Unchanged),
        Inline::state("unchanged", VisualState::Neutral)
    );
}

#[test]
fn every_sandbox_state_mode_and_placement_carries_its_own_explanation() -> Checked {
    // 状態値は翻訳しない。翻訳先で読めるのは凡例だけであり、表へ出した値には
    // その値を説明するmessageが要る。Sandboxの状態はhost serviceの説明を流用しない。
    let catalog = crate::i18n::Catalog::new(Locale::Ja);
    let mut legend = Legend::new(Locale::Ja);
    for state in [SandboxState::Running, SandboxState::Stopped] {
        assert_eq!(legend.sandbox_state(state), sandbox_state(state));
    }
    for mode in [CreationMode::Attached, CreationMode::Detached] {
        assert_eq!(legend.creation_mode(mode), creation_mode(mode));
    }
    for value in [Placement::Placed, Placement::Unchanged] {
        assert_eq!(legend.placement(value), placement(value));
    }

    let entries = legend.entries();
    let described: Vec<(&str, &str)> = entries
        .iter()
        .map(|entry| (entry.value.as_str(), entry.description.id))
        .collect();
    assert_eq!(
        described,
        vec![
            ("attached", "legend-attached"),
            ("detached", "legend-detached"),
            ("placed", "legend-placed"),
            ("running", "legend-sandbox-running"),
            ("stopped", "legend-sandbox-stopped"),
            ("unchanged", "legend-unchanged"),
        ]
    );
    for entry in &entries {
        let text = catalog
            .text(entry.description.id)
            .required_because(&format!("{} has a legend", entry.value))?;
        assert!(!text.is_empty(), "{} has an empty legend", entry.value);
    }
    Ok(())
}

#[test]
fn registering_a_cell_returns_it_unchanged() {
    let mut legend = Legend::new(Locale::Ja);
    let cell = legend.cell(
        project_state(ProjectState::Running),
        "legend-sandbox-running",
    );
    assert_eq!(cell, Inline::state("running", VisualState::Positive));
    assert_eq!(legend.entries().len(), 1);
}
