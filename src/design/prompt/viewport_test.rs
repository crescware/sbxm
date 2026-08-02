use std::fs::File;

use console::Term;
use rustix::fs::{Mode, OFlags};
use rustix::pty::{OpenptFlags, grantpt, openpt, ptsname, unlockpt};
use rustix::termios::{Winsize, tcsetwinsize};

use crate::testing::outcome::{Checked, Required};

use super::viewport;

/// 高さを答える端末と、その端末を持つ`Term`。
///
/// 端末の高さは実際の端末にしか無い。非TTYのpairでは、高さを読めない場合しか
/// 確かめられない。制御側は開いたまま持つ。閉じると端末が切れる。
fn attended(rows: u16) -> Checked<(File, Term)> {
    let controller = openpt(OpenptFlags::RDWR | OpenptFlags::NOCTTY)
        .required_because("a pseudo terminal is available")?;
    grantpt(&controller).required_because("the terminal side is usable")?;
    unlockpt(&controller).required_because("the terminal side is unlocked")?;
    let name = ptsname(&controller, Vec::new()).required_because("the terminal has a name")?;
    tcsetwinsize(
        &controller,
        Winsize {
            ws_row: rows,
            ws_col: 80,
            ws_xpixel: 0,
            ws_ypixel: 0,
        },
    )
    .required_because("the terminal takes its size")?;

    // 制御端末として奪わない。testを動かしているprocessのsessionへ結び付けない。
    let terminal = rustix::fs::open(&name, OFlags::RDWR | OFlags::NOCTTY, Mode::empty())
        .required_because("the terminal side opens")?;
    let write = File::from(terminal);
    let read = write
        .try_clone()
        .required_because("the same terminal is read and written")?;
    Ok((File::from(controller), Term::read_write_pair(read, write)))
}

/// 端末でない書き先。
fn unattended() -> Checked<Term> {
    let file = tempfile::tempfile().required_because("a writable file")?;
    let read = file
        .try_clone()
        .required_because("the same file is read and written")?;
    Ok(Term::read_write_pair(read, file))
}

#[test]
fn the_list_gives_back_the_rows_that_the_rest_of_the_prompt_needs() -> Checked {
    let (_controller, term) = attended(40)?;
    assert_eq!(
        viewport(&term),
        Some(34),
        "heading, keys, count, blank and result keep their rows"
    );
    Ok(())
}

#[test]
fn a_terminal_too_short_for_the_prompt_still_shows_one_candidate() -> Checked {
    let (_controller, term) = attended(4)?;
    assert_eq!(
        viewport(&term),
        Some(1),
        "a list of no rows would hide the selection entirely"
    );
    Ok(())
}

#[test]
fn a_stream_that_is_not_a_terminal_does_not_limit_the_list() -> Checked {
    assert_eq!(
        viewport(&unattended()?),
        None,
        "an unknown height is not a height of zero"
    );
    Ok(())
}
