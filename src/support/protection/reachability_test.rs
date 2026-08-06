use super::*;

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
