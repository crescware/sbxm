use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId};
use crate::msg;

use crate::testing::outcome::{Checked, Refused, Required};

use super::TransientArchive;

/// `template::ensure`が返しうる形のerror。
fn sample_error() -> Error {
    Error::single(Diagnostic::new(
        ErrorId::ExternalCommandFailed,
        msg!(
            "error-external-command-failed",
            exit_status = "exit status: 1"
        ),
    ))
}

fn has_path_fact(facts: &[Fact], path: &str) -> bool {
    facts.iter().any(|fact| {
        matches!(fact, Fact::OneLine { label, value }
            if label.id == "diagnostic-path-label" && value.as_str() == path)
    })
}

fn has_cause_fact(facts: &[Fact]) -> bool {
    facts.iter().any(
        |fact| matches!(fact, Fact::OneLine { label, .. } if label.id == "diagnostic-cause-label"),
    )
}

#[test]
fn a_successful_outcome_is_kept_once_the_file_is_removed() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let path = dir.path().join("template-000000000000.tar");
    std::fs::write(&path, b"archive").required()?;
    let archive = TransientArchive::new(path.clone());
    let mut warnings = Vec::new();

    let outcome = archive.cleanup_after(Ok(42), &mut warnings);

    assert_eq!(
        outcome.required_because("cleanup does not change a successful outcome")?,
        42
    );
    assert!(warnings.is_empty(), "{warnings:?}");
    assert!(
        !path.exists(),
        "the archive is removed once it is no longer needed"
    );
    Ok(())
}

#[test]
fn a_failed_outcome_is_kept_once_the_file_is_removed() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let path = dir.path().join("template-000000000000.tar");
    std::fs::write(&path, b"archive").required()?;
    let archive = TransientArchive::new(path.clone());
    let mut warnings = Vec::new();
    let original = sample_error();

    let outcome = archive.cleanup_after(Err::<(), Error>(original.clone()), &mut warnings);

    assert_eq!(
        outcome.refused_because("cleanup does not turn a failed load into a success")?,
        original
    );
    assert!(warnings.is_empty(), "{warnings:?}");
    assert!(
        !path.exists(),
        "the archive is removed even though the load failed"
    );
    Ok(())
}

#[test]
fn a_cleanup_failure_after_success_is_a_warning_rather_than_a_replaced_result() -> Checked {
    // `remove_file`はdirectoryを拒む。fileの代わりにdirectoryを渡し、削除自体を失敗させる。
    let dir = tempfile::tempdir().required()?;
    let path = dir.path().join("template-000000000000.tar");
    std::fs::create_dir(&path).required()?;
    let archive = TransientArchive::new(path.clone());
    let mut warnings = Vec::new();

    let outcome = archive.cleanup_after(Ok(42), &mut warnings);

    assert_eq!(
        outcome.required_because("a cleanup failure never hides the value that was produced")?,
        42
    );
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert_eq!(warnings[0].description.id, "warning-archive-cleanup-failed");
    assert!(
        has_path_fact(&warnings[0].facts, &crate::paths::display(&path)),
        "the archive that could not be removed is named: {:?}",
        warnings[0].facts
    );
    assert!(
        has_cause_fact(&warnings[0].facts),
        "the operating system's own reason is carried: {:?}",
        warnings[0].facts
    );
    Ok(())
}

#[test]
fn a_cleanup_failure_after_a_load_failure_is_folded_into_the_original_error() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let path = dir.path().join("template-000000000000.tar");
    std::fs::create_dir(&path).required()?;
    let archive = TransientArchive::new(path.clone());
    let mut warnings = Vec::new();
    let original = sample_error();

    let outcome = archive.cleanup_after(Err::<(), Error>(original.clone()), &mut warnings);

    let decorated = outcome.refused_because("the load itself already failed")?;
    assert!(warnings.is_empty(), "{warnings:?}");
    let (before, after) = (&original.diagnostics()[0], &decorated.diagnostics()[0]);
    assert_eq!(after.id, before.id);
    assert_eq!(after.description, before.description);
    assert_eq!(after.remediation, before.remediation);
    assert_eq!(after.external, before.external);
    assert!(before.facts.is_empty(), "{:?}", before.facts);
    assert!(
        has_path_fact(&after.facts, &crate::paths::display(&path)),
        "the archive that could not be removed is named: {:?}",
        after.facts
    );
    assert!(
        has_cause_fact(&after.facts),
        "the operating system's own reason is carried: {:?}",
        after.facts
    );
    Ok(())
}

#[test]
fn a_cleanup_failure_after_a_cancellation_leaves_it_a_cancellation() -> Checked {
    // `Error::Canceled`は診断を持たないため、事実を追記する先がない。そのまま返す。
    let dir = tempfile::tempdir().required()?;
    let path = dir.path().join("template-000000000000.tar");
    std::fs::create_dir(&path).required()?;
    let archive = TransientArchive::new(path.clone());
    let mut warnings = Vec::new();

    let outcome = archive.cleanup_after(Err::<(), Error>(Error::Canceled), &mut warnings);

    assert_eq!(
        outcome.refused_because("a cancellation stays a cancellation")?,
        Error::Canceled
    );
    assert!(warnings.is_empty(), "{warnings:?}");
    Ok(())
}

#[test]
fn dropping_an_archive_that_was_never_cleaned_up_removes_it() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let path = dir.path().join("template-000000000000.tar");
    std::fs::write(&path, b"archive").required()?;

    drop(TransientArchive::new(path.clone()));

    assert!(!path.exists(), "the last resort cleanup runs on drop");
    Ok(())
}
