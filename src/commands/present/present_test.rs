use crate::commands::status::project::Value as ProjectValue;
use crate::commands::stop::StopResult;
use crate::design::{Inline, VisualState};
use crate::i18n::Locale;
use crate::metadata::CreationMode;
use crate::support::inventory::ProjectState;
use crate::support::status::StatusValue;

use crate::testing::outcome::{Checked, Required};

use super::*;

/// project scopeが表に出す状態値のすべて。
const PROJECT_VALUES: [ProjectValue; 15] = [
    ProjectValue::Ready,
    ProjectValue::Missing,
    ProjectValue::Mismatch,
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
fn registering_a_cell_returns_it_unchanged() {
    let mut legend = Legend::new(Locale::Ja);
    let cell = legend.cell(
        project_state(ProjectState::Running),
        "legend-sandbox-running",
    );
    assert_eq!(cell, Inline::state("running", VisualState::Positive));
    assert_eq!(legend.entries().len(), 1);
}
