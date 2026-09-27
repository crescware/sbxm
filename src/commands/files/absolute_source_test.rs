use std::path::{Path, PathBuf};

use crate::design::Fact;
use crate::diagnostics::ErrorId;
use crate::testing::outcome::{Checked, Refused, Required};

use super::absolute_source_from;

/// 消されたdirectoryの中で実行したときのように、current directoryを読めない。
fn deleted() -> std::io::Result<PathBuf> {
    Err(std::io::Error::from(std::io::ErrorKind::NotFound))
}

#[test]
fn a_relative_source_is_refused_when_the_current_directory_cannot_be_read() -> Checked {
    let error = absolute_source_from(Path::new("notes.md"), &deleted)
        .refused_because("a relative path has nothing to be resolved from")?;

    assert_eq!(error.first_id(), Some(ErrorId::DeclaredFileUnusable));
    let facts = &error
        .diagnostics()
        .first()
        .required_because("the refusal carries a diagnostic")?
        .facts;
    // 渡されたままの綴りと、OSが書いた原因を示す。
    assert!(
        facts.contains(&Fact::source("notes.md")),
        "the given spelling is named: {facts:?}"
    );
    assert!(
        facts.contains(&Fact::cause(
            &std::io::Error::from(std::io::ErrorKind::NotFound).to_string()
        )),
        "the cause is stated as the OS wrote it: {facts:?}"
    );
    Ok(())
}

#[test]
fn an_absolute_source_does_not_ask_for_the_current_directory() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let real = std::fs::canonicalize(dir.path()).required()?;

    let resolved =
        absolute_source_from(&real.join("notes.md"), &deleted).required_because("absolute")?;

    assert_eq!(resolved, real.join("notes.md"));
    Ok(())
}
