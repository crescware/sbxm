use std::io;

use crate::testing::command::{Event, ScriptedOs, Step};
use crate::testing::outcome::{Checked, Refused, Required};

use super::*;

/// testの`refused_because`が`Ok`側を示すために使う。本番のcodeは`SignalGuard`を表示しない。
impl<S: super::Signals> std::fmt::Debug for SignalGuard<'_, S> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SignalGuard")
            .finish_non_exhaustive()
    }
}

#[test]
fn a_guard_watches_for_interrupts_until_it_is_dropped() -> Checked {
    let os = ScriptedOs::default();
    let guard = SignalGuard::new(&os).required_because("the watch is placed")?;
    assert_eq!(os.events(), [Event::WatchingInterrupts]);
    assert!(!guard.interrupted(), "nothing has arrived yet");

    drop(guard);
    assert_eq!(
        os.events(),
        [Event::WatchingInterrupts, Event::StoppedWatching]
    );
    Ok(())
}

#[test]
fn a_guard_whose_watch_cannot_be_placed_is_refused() -> Checked {
    let os = ScriptedOs::default().failing(
        Step::WatchInterrupts,
        io::Error::other("the handler could not be installed"),
    );
    let error = SignalGuard::new(&os).refused_because("no watch was placed")?;
    assert_eq!(error.to_string(), "the handler could not be installed");
    assert!(os.events().is_empty(), "nothing is left to remove");
    Ok(())
}

#[test]
fn an_interrupt_that_already_arrived_is_seen_at_once() -> Checked {
    let os = ScriptedOs::default().interrupting_at_poll(0);
    let guard = SignalGuard::new(&os).required_because("the watch is placed")?;
    assert!(guard.interrupted());
    Ok(())
}
