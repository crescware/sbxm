use console::Key;

use crate::metadata::MAX_WORKTREE_INDEX;

use super::super::{Action, OpenSelection, Transition, action_for};

#[test]
fn vertical_keys_move_projects_and_horizontal_keys_change_index() {
    let mut selection = OpenSelection::new(3, 4);

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
    let mut selection = OpenSelection::new(2, 1);

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
fn a_calculated_maximum_clamps_the_current_index_without_affecting_other_projects() {
    let mut selection = OpenSelection::new(2, MAX_WORKTREE_INDEX);
    selection.apply(Action::IncreaseIndex);
    selection.apply(Action::IncreaseIndex);
    selection.set_maximum(0, 1);

    assert_eq!(selection.current_index(), 1);
    assert_eq!(selection.maximum_index(), 1);

    selection.apply(Action::Next);
    assert_eq!(selection.current_index(), 1);
    assert_eq!(selection.maximum_index(), MAX_WORKTREE_INDEX);
}

#[test]
fn enter_confirms_both_current_values() {
    let mut selection = OpenSelection::new(3, MAX_WORKTREE_INDEX);
    selection.apply(action_for(&Key::ArrowDown));
    selection.apply(action_for(&Key::ArrowRight));
    selection.apply(action_for(&Key::ArrowRight));

    assert_eq!(
        selection.apply(action_for(&Key::Enter)),
        Transition::DoneOpen {
            project: 1,
            index: 2,
        }
    );
}

#[test]
fn a_prompt_with_no_projects_does_not_move_or_confirm_a_project() {
    let mut selection = OpenSelection::new(0, MAX_WORKTREE_INDEX);
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
