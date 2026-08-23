use console::Term;

use crate::design::prompt::{Key, Keys, Screen};
use crate::testing::outcome::{Checked, Required};

use super::RealTerminal;

#[test]
fn a_screen_that_is_not_a_terminal_reports_no_height_and_no_keys() -> Checked {
    // 端末でない先へ書くこと自体は妨げない。読めないのは高さと打鍵である。
    let directory = tempfile::tempdir().required_because("a temporary directory")?;
    let path = directory.path().join("screen");
    let write = std::fs::File::create(&path).required_because("a writable screen")?;
    let read = std::fs::File::open(&path).required_because("a readable screen")?;
    let mut terminal = RealTerminal::new(Term::read_write_pair(read, write));

    assert_eq!(
        terminal.rows(),
        None,
        "the default size is not an observed one"
    );
    assert_eq!(
        terminal.read_key().required_because("a key")?,
        Key::Unknown,
        "nobody is there to press anything"
    );

    terminal.write_line("alpha").required_because("a line")?;
    terminal
        .write_str("brav")
        .required_because("part of a line")?;
    assert_eq!(
        std::fs::read_to_string(&path).required_because("what was written")?,
        "alpha\nbrav"
    );
    Ok(())
}
