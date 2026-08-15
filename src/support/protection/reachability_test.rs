use std::collections::{BTreeMap, BTreeSet};

use super::*;

const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";

fn candidate(upstream: Option<&str>) -> CommitCandidate {
    CommitCandidate::new(
        "refs/heads/main".to_string(),
        COMMIT.to_string(),
        upstream.map(str::to_string),
    )
}

fn observed(reachable_from: BTreeMap<String, BTreeSet<String>>) -> OriginObservation {
    OriginObservation::Observed {
        tips: BTreeMap::new(),
        reachable_from,
    }
}

#[test]
fn a_commit_reachable_from_its_upstream_is_pushed() {
    let observation = observed(BTreeMap::from([(
        COMMIT.to_string(),
        BTreeSet::from([
            "refs/remotes/origin/main".to_string(),
            "refs/remotes/origin/release".to_string(),
        ]),
    )]));
    let candidate = candidate(Some("refs/remotes/origin/main"));

    assert_eq!(
        Reachability::classify(&candidate, &observation),
        Reachability::Pushed {
            upstream: "refs/remotes/origin/main".to_string(),
        }
    );
}

#[test]
fn a_commit_reachable_only_from_a_non_upstream_ref_is_reachable() {
    let observation = observed(BTreeMap::from([(
        COMMIT.to_string(),
        BTreeSet::from(["refs/remotes/origin/release".to_string()]),
    )]));
    let candidate = candidate(Some("refs/remotes/origin/main"));

    assert_eq!(
        Reachability::classify(&candidate, &observation),
        Reachability::Reachable {
            origins: vec!["refs/remotes/origin/release".to_string()],
        }
    );
}

#[test]
fn a_commit_reachable_from_no_origin_ref_is_unreachable() {
    let observation = observed(BTreeMap::from([(COMMIT.to_string(), BTreeSet::new())]));
    let candidate = candidate(None);

    assert_eq!(
        Reachability::classify(&candidate, &observation),
        Reachability::Unreachable
    );
}

#[test]
fn a_commit_missing_from_the_observation_is_unobservable_not_unreachable() {
    let observation = observed(BTreeMap::new());
    let candidate = candidate(None);

    assert_eq!(
        Reachability::classify(&candidate, &observation),
        Reachability::Unobservable {
            reason: UnobservableReason::ObjectMissing,
        }
    );
}

#[test]
fn every_state_has_its_own_untranslated_spelling_and_legend() {
    let states = [
        (
            Reachability::Pushed {
                upstream: "refs/remotes/origin/main".to_string(),
            },
            "pushed",
            "legend-pushed",
        ),
        (
            Reachability::Reachable {
                origins: vec!["refs/remotes/origin/release".to_string()],
            },
            "reachable",
            "legend-reachable",
        ),
        (
            Reachability::Unreachable,
            "unreachable",
            "legend-unreachable",
        ),
        (
            Reachability::Unobservable {
                reason: UnobservableReason::RefreshFailed,
            },
            "unobservable",
            "legend-unobservable",
        ),
    ];
    for (state, spelling, legend) in states {
        assert_eq!(state.as_str(), spelling);
        assert_eq!(state.legend_id(), legend);
    }
}

#[test]
fn an_unobservable_state_displays_its_reason_without_changing_its_legend() {
    let state = Reachability::Unobservable {
        reason: UnobservableReason::ReadOnlyDataInsufficient,
    };

    assert_eq!(
        state.display(),
        "unobservable(read-only-data-insufficient)".to_string()
    );
    assert_eq!(state.as_str(), "unobservable");
    assert_eq!(state.legend_id(), "legend-unobservable");
}
