//! 待ち合わせの上限と、待たずに返る場合。台本の時計で確かめ、実時間を待たない。

use std::time::Duration;

use crate::testing::outcome::{Checked, Refused, Unmet};
use crate::testing::scripted_clock::ScriptedClock;

use super::*;

#[test]
fn a_state_that_is_already_there_is_returned_without_waiting() -> Checked {
    let clock = ScriptedClock::default();
    assert_eq!(wait_until(&clock, "the value", || Ok(Some(7)))?, 7);
    assert!(clock.slept.borrow().is_empty());
    Ok(())
}

#[test]
fn a_state_that_arrives_later_is_rechecked_until_it_is_there() -> Checked {
    let clock = ScriptedClock::default();
    let mut looks = 0;
    let found = wait_until(&clock, "the third look", || {
        looks += 1;
        Ok((looks == 3).then_some(looks))
    })?;
    assert_eq!(found, 3);
    assert_eq!(*clock.slept.borrow(), [RECHECK, RECHECK]);
    Ok(())
}

#[test]
fn a_state_that_never_arrives_fails_once_the_limit_has_passed() -> Checked {
    let clock = ScriptedClock::default();
    let unmet = wait_until(&clock, "the marker", || Ok(None::<()>))
        .refused_because("the marker never appears")?;
    assert!(
        unmet
            .reason
            .contains("the marker did not happen within 60 seconds"),
        "{unmet}"
    );
    // 上限まで待ってから諦める。上限より先に諦めれば、遅い環境で整う状態を失敗と読む。
    let waited: Duration = clock.slept.borrow().iter().sum();
    assert_eq!(waited, WAIT_LIMIT);
    Ok(())
}

#[test]
fn a_look_that_fails_ends_the_wait_with_its_own_reason() -> Checked {
    let clock = ScriptedClock::default();
    let mut looks = 0;
    let unmet = wait_until(&clock, "the pipe", || {
        looks += 1;
        if looks < 3 {
            Ok(None::<()>)
        } else {
            Err(Unmet::new("the pipe broke"))
        }
    })
    .refused_because("the third look fails")?;
    assert_eq!(unmet.reason, "the pipe broke");
    assert_eq!(clock.slept.borrow().len(), 2);
    Ok(())
}
