//! 契約test: OSの単調な時計についての仮定。
//!
//! 判断のcodeは、時計が戻らないことと、待った時間が少なくとも頼んだ長さであることを前提に
//! 期限を数える。速さは見ない。

use std::time::Duration;

use crate::time::Clock;

use super::*;

#[test]
fn the_clock_does_not_go_back() {
    let before = SystemClock.now();
    let after = SystemClock.now();
    assert!(after >= before, "{before:?} then {after:?}");
}

#[test]
fn a_sleep_lasts_at_least_as_long_as_asked() {
    let asked = Duration::from_millis(1);
    let before = SystemClock.now();
    SystemClock.sleep(asked);
    let after = SystemClock.now();
    assert!(
        after >= before.after(asked),
        "asked {asked:?}: {before:?} then {after:?}"
    );
}
