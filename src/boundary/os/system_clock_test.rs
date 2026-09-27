//! 契約test: OSの時計についての仮定。
//!
//! 判断のcodeは、時計が戻らないことと、待った時間が少なくとも頼んだ長さであることを前提に
//! 期限を数える。速さは見ない。名前に刻む時刻は、壁時計がUNIX epochより後を指すことを前提に
//! する。

use std::time::{Duration, UNIX_EPOCH};

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

#[test]
fn the_wall_clock_is_after_the_epoch() {
    // `stamp`はepochより前を0へ丸める。丸めた名前は、時刻の順に並ばない。
    let wall = SystemClock.wall();
    assert!(wall > UNIX_EPOCH, "{wall:?}");
}
