use super::super::{Action, Key, OpenSelection, Transition, action_for};

fn labels(count: usize) -> Vec<String> {
    ["owner/alpha", "owner/bravo", "owner/charlie"]
        .iter()
        .take(count)
        .map(|label| (*label).to_string())
        .collect()
}

#[test]
fn vertical_keys_move_projects_and_horizontal_keys_change_index() {
    let mut selection = OpenSelection::new(&labels(3), &[Some(4); 3]);

    assert_eq!(selection.apply(Action::Next), Transition::Continue);
    assert_eq!(selection.current_project(), 1);
    assert_eq!(selection.apply(Action::IncreaseIndex), Transition::Continue);
    assert_eq!(selection.current_index(), 1);
    assert_eq!(selection.apply(Action::IncreaseIndex), Transition::Continue);
    assert_eq!(selection.current_index(), 2);
    assert_eq!(selection.apply(Action::Previous), Transition::Continue);
    assert_eq!(selection.current_project(), 0);
    assert_eq!(selection.current_index(), 2);
}

#[test]
fn both_axes_are_clamped_or_wrapped_by_their_own_rules() {
    let mut selection = OpenSelection::new(&labels(2), &[Some(1); 2]);

    assert_eq!(selection.apply(Action::DecreaseIndex), Transition::Continue);
    assert_eq!(selection.current_index(), 0);
    assert_eq!(selection.apply(Action::IncreaseIndex), Transition::Continue);
    assert_eq!(selection.apply(Action::IncreaseIndex), Transition::Continue);
    assert_eq!(selection.current_index(), 1);
    assert_eq!(selection.apply(Action::Previous), Transition::Continue);
    assert_eq!(selection.current_project(), 1);
    assert_eq!(selection.apply(Action::Next), Transition::Continue);
    assert_eq!(selection.current_project(), 0);
}

#[test]
fn moving_to_a_project_with_fewer_worktrees_brings_the_index_within_it() {
    let mut selection = OpenSelection::new(&labels(2), &[Some(3), Some(1)]);
    for _ in 0..3 {
        selection.apply(Action::IncreaseIndex);
    }
    assert_eq!(selection.current_index(), 3);

    selection.apply(Action::Next);
    assert_eq!(
        selection.current_index(),
        1,
        "the index is held within the project the cursor moved to"
    );
    assert_eq!(selection.maximum_index(), Some(1));

    selection.apply(Action::Previous);
    assert_eq!(
        selection.current_index(),
        1,
        "the index given up on the way is not restored"
    );
    assert_eq!(selection.maximum_index(), Some(3));
}

#[test]
fn a_project_whose_metadata_could_not_be_read_keeps_its_index_at_zero() {
    let mut selection = OpenSelection::new(&labels(1), &[None]);
    selection.apply(Action::IncreaseIndex);
    selection.apply(Action::IncreaseIndex);

    assert_eq!(
        selection.current_index(),
        0,
        "a range that was not read does not let the index move"
    );
    assert_eq!(selection.maximum_index(), None);
}

#[test]
fn a_project_without_a_given_maximum_has_no_range() {
    let mut selection = OpenSelection::new(&labels(2), &[Some(2)]);
    selection.apply(Action::Next);
    selection.apply(Action::IncreaseIndex);

    assert_eq!(
        selection.maximum_index(),
        None,
        "a missing entry is not read out of another project's slot"
    );
    assert_eq!(selection.current_index(), 0);
}

#[test]
fn enter_confirms_both_current_values() {
    let mut selection = OpenSelection::new(&labels(3), &[Some(4); 3]);
    selection.apply(action_for(Key::ArrowDown));
    selection.apply(action_for(Key::ArrowRight));
    selection.apply(action_for(Key::ArrowRight));

    assert_eq!(
        selection.apply(action_for(Key::Enter)),
        Transition::DoneOpen {
            project: 1,
            index: 2,
        }
    );
}

#[test]
fn a_prompt_with_no_projects_does_not_move_or_confirm_a_project() {
    let mut selection = OpenSelection::new(&[], &[]);
    assert_eq!(selection.apply(Action::Next), Transition::Continue);
    assert_eq!(selection.current_project(), 0);
    assert_eq!(
        selection.apply(Action::Confirm),
        Transition::DoneOpen {
            project: 0,
            index: 0,
        }
    );
}

#[test]
fn keys_the_open_prompt_does_not_use_leave_both_values_alone() {
    // Spaceで選ぶ候補は無い。受け付けない打鍵と同じく、案件もindexも動かさない。
    let mut selection = OpenSelection::new(&labels(2), &[Some(3); 2]);
    selection.apply(Action::Next);
    selection.apply(Action::IncreaseIndex);

    for action in [Action::Toggle, Action::Ignore] {
        assert_eq!(selection.apply(action), Transition::Continue, "{action:?}");
        assert_eq!(selection.current_project(), 1, "{action:?}");
        assert_eq!(selection.current_index(), 1, "{action:?}");
    }
}

#[test]
fn the_index_follows_the_project_the_filter_moves_the_focus_to() {
    let mut selection = OpenSelection::new(&labels(2), &[Some(3), Some(1)]);
    for _ in 0..3 {
        selection.apply(Action::IncreaseIndex);
    }
    selection.apply(Action::Type('b'));
    assert_eq!(selection.current_project(), 1);
    assert_eq!(
        selection.current_index(),
        1,
        "the index is held within the project the filter landed on"
    );
}

#[test]
fn a_filter_that_matches_no_project_refuses_to_confirm_until_it_is_erased() {
    let mut selection = OpenSelection::new(&labels(3), &[Some(4); 3]);
    selection.apply(Action::Next);
    selection.apply(Action::Type('z'));
    assert!(selection.visible().is_empty());
    assert_eq!(
        selection.current_project(),
        1,
        "the focused project is remembered while nothing matches"
    );
    assert!(!selection.warns_about_no_match());

    assert_eq!(
        selection.apply(Action::Confirm),
        Transition::Continue,
        "nothing hidden is confirmed by accident"
    );
    assert!(selection.warns_about_no_match());

    assert_eq!(selection.apply(Action::Erase), Transition::Continue);
    assert!(
        !selection.warns_about_no_match(),
        "erasing takes the warning back down"
    );
    assert_eq!(selection.visible(), vec![0, 1, 2]);
    assert_eq!(
        selection.apply(Action::Confirm),
        Transition::DoneOpen {
            project: 1,
            index: 0,
        },
        "the remembered project is the one confirmed"
    );
}

#[test]
fn erasing_an_empty_filter_changes_nothing() {
    let mut selection = OpenSelection::new(&labels(2), &[Some(1); 2]);
    selection.apply(Action::Next);
    let before = selection.clone();
    assert_eq!(selection.apply(Action::Erase), Transition::Continue);
    assert_eq!(selection, before);
}

#[test]
fn typing_narrows_the_projects_and_the_arrows_move_among_the_matches() {
    let mut selection = OpenSelection::new(&labels(3), &[Some(4); 3]);
    selection.apply(Action::Next);
    assert_eq!(selection.apply(Action::Type('h')), Transition::Continue);
    assert_eq!(selection.narrowing().query(), "h");
    assert_eq!(
        selection.visible(),
        vec![0, 2],
        "alpha and charlie contain the letter"
    );
    assert_eq!(
        selection.current_project(),
        0,
        "the hidden project hands the focus to the first match"
    );

    selection.apply(Action::Next);
    assert_eq!(
        selection.current_project(),
        2,
        "the hidden project in between is skipped"
    );
    selection.apply(Action::Next);
    assert_eq!(
        selection.current_project(),
        0,
        "the arrows cycle through the matches only"
    );
    selection.apply(Action::Previous);
    assert_eq!(selection.current_project(), 2);

    assert_eq!(
        selection.apply(Action::Confirm),
        Transition::DoneOpen {
            project: 2,
            index: 0,
        },
        "the confirmed project is the candidate index, not the row"
    );
}
