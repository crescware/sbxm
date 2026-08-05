use console::Key;

use super::*;
use crate::design::prompt::action_for;

#[test]
fn left_and_right_are_clamped_to_the_index_range() {
    let mut selection = IndexSelection::new(4);

    assert_eq!(
        selection.apply(action_for(&Key::ArrowLeft)),
        Transition::Continue
    );
    assert_eq!(selection.current(), 0);

    for expected in 1..=4 {
        assert_eq!(
            selection.apply(action_for(&Key::ArrowRight)),
            Transition::Continue
        );
        assert_eq!(selection.current(), expected);
    }

    assert_eq!(
        selection.apply(action_for(&Key::ArrowRight)),
        Transition::Continue
    );
    assert_eq!(selection.current(), 4);

    for expected in (0..4).rev() {
        assert_eq!(
            selection.apply(action_for(&Key::ArrowLeft)),
            Transition::Continue
        );
        assert_eq!(selection.current(), expected);
    }
}

#[test]
fn a_single_worktree_has_only_index_zero() {
    let mut selection = IndexSelection::new(0);
    selection.apply(Action::IncreaseIndex);
    assert_eq!(selection.current(), 0);
    selection.apply(Action::DecreaseIndex);
    assert_eq!(selection.current(), 0);
    assert_eq!(selection.apply(Action::Confirm), Transition::DoneIndex(0));
}

#[test]
fn unrelated_keys_do_not_change_the_index() {
    let mut selection = IndexSelection::new(4);
    selection.apply(Action::IncreaseIndex);
    let before = selection;
    for key in [Key::ArrowUp, Key::ArrowDown, Key::Char(' '), Key::Tab] {
        assert_eq!(selection.apply(action_for(&key)), Transition::Continue);
    }
    assert_eq!(selection, before);
}
