use std::path::PathBuf;

use crate::design::{RenderingPolicy, Ui};
use crate::i18n::Locale;
use crate::support::host_sync::RefChange;

use crate::testing::outcome::{Checked, Required};

use super::super::SaveOutput;
use super::*;

fn rendered(output: &SaveOutput, locale: Locale) -> Checked<String> {
    let mut written: Vec<u8> = Vec::new();
    {
        let mut ui = Ui::capture(
            locale,
            RenderingPolicy::plain(),
            &mut written,
            std::io::sink(),
        );
        ui.stdout(&saved_document(output, locale));
    }
    String::from_utf8(written).required_because("UTF-8")
}

fn output(changes: Option<Vec<RefChange>>) -> SaveOutput {
    SaveOutput {
        project: "Example-Org/Example-Repo".to_string(),
        repository: PathBuf::from("/Users/example/Projects/example-repo.project/example-repo"),
        namespace: "sbxm-example".to_string(),
        changes,
    }
}

#[test]
fn every_changed_ref_is_listed_with_where_its_old_tip_went() -> Checked {
    let text = rendered(
        &output(Some(vec![
            RefChange::Created {
                reference: "refs/sbx/sbxm-example/heads/topic".to_string(),
            },
            RefChange::Updated {
                reference: "refs/sbx/sbxm-example/heads/main".to_string(),
            },
            RefChange::Replaced {
                reference: "refs/sbx/sbxm-example/heads/rewritten".to_string(),
                archived: "refs/sbx/sbxm-example/archive/20260923T100000Z/heads/rewritten"
                    .to_string(),
            },
            RefChange::Deleted {
                reference: "refs/sbx/sbxm-example/heads/gone".to_string(),
                archived: "refs/sbx/sbxm-example/archive/20260923T100000Z/heads/gone".to_string(),
            },
        ])),
        Locale::En,
    )?;
    assert!(text.contains("refs/sbx/sbxm-example"), "{text}");
    let row = text
        .lines()
        .find(|line| line.contains("heads/rewritten"))
        .required_because("the replaced ref has a row")?;
    assert!(
        row.contains("replaced") && row.contains("archive/20260923T100000Z/heads/rewritten"),
        "{row}"
    );
    for value in ["created", "updated", "deleted"] {
        assert!(text.contains(value), "{value}: {text}");
    }
    assert!(text.contains("left as they were"), "{text}");

    // 日本語では各値に凡例が付く。
    let text = rendered(
        &output(Some(vec![RefChange::Created {
            reference: "refs/sbx/sbxm-example/heads/topic".to_string(),
        }])),
        Locale::Ja,
    )?;
    assert!(text.contains("refを作りました"), "{text}");
    Ok(())
}

#[test]
fn nothing_new_and_nothing_to_save_are_told_apart() -> Checked {
    assert!(rendered(&output(Some(Vec::new())), Locale::En)?.contains("already holds"));
    assert!(rendered(&output(None), Locale::En)?.contains("no branch or tag"));
    Ok(())
}
