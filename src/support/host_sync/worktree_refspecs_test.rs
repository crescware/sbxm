use super::*;
use crate::testing::outcome::{Checked, Refused, Required};
use crate::testing::value::COMMIT;

#[test]
fn unusual_and_duplicate_names_keep_every_head_without_creating_source_refs() -> Checked {
    let record = |path: &str| format!("worktree {path}\0HEAD {COMMIT}\0detached\0\0");
    let text = format!(
        "{}{}{}{}",
        record("/a/shared"),
        record("/b/shared"),
        record("/a/line\nbreak"),
        record("/a/.lock")
    );
    let specs = refspecs(&text, "refs/in/").required()?;
    assert_eq!(specs.len(), 4);
    let unique: BTreeSet<_> = specs.iter().collect();
    assert_eq!(unique.len(), 4);
    assert!(
        specs
            .iter()
            .all(|s| s.starts_with(&format!("+{COMMIT}:refs/in/worktrees/")))
    );
    for bad in [
        "",
        "worktree /a\0detached\0\0",
        "worktree /a\0HEAD bad\0detached\0\0",
    ] {
        refspecs(bad, "refs/in/")
            .refused_because("a missing head must not disappear from a save")?;
    }
    Ok(())
}
