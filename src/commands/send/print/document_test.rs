use std::path::PathBuf;

use crate::design::{RenderingPolicy, Ui};
use crate::i18n::Locale;

use crate::testing::outcome::{Checked, Required};

use super::*;
use crate::commands::send::SendOutput;
use crate::commands::sync::SentChange;

fn rendered(output: &SendOutput, locale: Locale) -> Checked<String> {
    let mut written: Vec<u8> = Vec::new();
    {
        let mut ui = Ui::capture(
            locale,
            RenderingPolicy::plain(),
            &mut written,
            std::io::sink(),
        );
        ui.stdout(&document(output, locale));
    }
    String::from_utf8(written).required_because("UTF-8")
}

fn output(changes: Vec<SentChange>) -> SendOutput {
    SendOutput {
        project: "local/app".to_string(),
        repository: PathBuf::from("/Users/example/code/app"),
        changes,
    }
}

#[test]
fn every_ref_that_changed_in_the_sandbox_origin_is_listed() -> Checked {
    let text = rendered(
        &output(vec![
            SentChange::Created {
                reference: "refs/remotes/origin/topic".to_string(),
            },
            SentChange::Updated {
                reference: "refs/remotes/origin/main".to_string(),
            },
            SentChange::Removed {
                reference: "refs/remotes/origin/old".to_string(),
            },
        ]),
        Locale::En,
    )?;
    assert!(
        text.starts_with("\u{2713} Sent the branches and tags of /Users/example/code/app"),
        "{text}"
    );
    for expected in [
        "refs/remotes/origin/topic",
        "created",
        "updated",
        "removed",
        "worktrees and branches in the sandbox were left as they were",
    ] {
        assert!(text.contains(expected), "{expected}: {text}");
    }
    Ok(())
}

#[test]
fn a_send_that_changed_nothing_says_so() -> Checked {
    let text = rendered(&output(Vec::new()), Locale::Ja)?;
    assert!(text.contains("すでにすべて持っています"), "{text}");
    Ok(())
}
