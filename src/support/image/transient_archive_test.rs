use crate::design::{ExternalOutput, Fact, ProgressSink, SilentProgress, Warning};
use crate::diagnostics::{Diagnostic, Error, ErrorId, Msg};
use crate::msg;

use crate::testing::outcome::{Checked, Refused, Required};

use super::TransientArchive;

/// `progress.warn`へ渡された内容を記録するだけのsink。
#[derive(Default)]
struct RecordingProgress {
    warnings: Vec<Warning>,
}

impl ProgressSink for RecordingProgress {
    fn step(&mut self, _message: Msg) {}

    fn warn(&mut self, warning: Warning) {
        self.warnings.push(warning);
    }
}

impl ExternalOutput for RecordingProgress {
    fn relay(&mut self, _bytes: &[u8]) {}
    fn hand_over(&mut self) {}
    fn finished(&mut self) {}
}

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
    let archive = TransientArchive::new(path.clone()).required()?;
    let mut warnings = Vec::new();

    let outcome = archive.cleanup_after(Ok(42), &mut warnings, &mut SilentProgress);

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
    let archive = TransientArchive::new(path.clone()).required()?;
    let mut warnings = Vec::new();
    let original = sample_error();

    let outcome = archive.cleanup_after(
        Err::<(), Error>(original.clone()),
        &mut warnings,
        &mut SilentProgress,
    );

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
    let archive = TransientArchive::new(path.clone()).required()?;
    let mut warnings = Vec::new();

    let outcome = archive.cleanup_after(Ok(42), &mut warnings, &mut SilentProgress);

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
    let archive = TransientArchive::new(path.clone()).required()?;
    let mut warnings = Vec::new();
    let original = sample_error();

    let outcome = archive.cleanup_after(
        Err::<(), Error>(original.clone()),
        &mut warnings,
        &mut SilentProgress,
    );

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
fn a_cleanup_failure_after_a_cancellation_is_reported_through_progress() -> Checked {
    // `Error::Canceled`は診断を持たないため、事実を追記する先がない。exit codeは
    // cancellationのまま保つが、archiveが残った事実はprogressへその場で伝える。
    let dir = tempfile::tempdir().required()?;
    let path = dir.path().join("template-000000000000.tar");
    std::fs::create_dir(&path).required()?;
    let archive = TransientArchive::new(path.clone()).required()?;
    let mut warnings = Vec::new();
    let mut progress = RecordingProgress::default();

    let outcome = archive.cleanup_after(
        Err::<(), Error>(Error::Canceled),
        &mut warnings,
        &mut progress,
    );

    assert_eq!(
        outcome.refused_because("a cancellation stays a cancellation")?,
        Error::Canceled
    );
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(progress.warnings.len(), 1, "{:?}", progress.warnings);
    assert_eq!(
        progress.warnings[0].description.id,
        "warning-archive-cleanup-failed"
    );
    assert!(
        has_path_fact(&progress.warnings[0].facts, &crate::paths::display(&path)),
        "the archive that could not be removed is named: {:?}",
        progress.warnings[0].facts
    );
    assert!(
        has_cause_fact(&progress.warnings[0].facts),
        "the operating system's own reason is carried: {:?}",
        progress.warnings[0].facts
    );
    Ok(())
}

#[test]
fn a_replaced_file_is_left_alone_instead_of_removed() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let path = dir.path().join("template-000000000000.tar");
    std::fs::write(&path, b"archive").required()?;
    let archive = TransientArchive::new(path.clone()).required()?;
    // loadの間に、この実行が作った実体が別のものへ入れ替わった状況を模す。`rename`で
    // 差し替えることで、削除・再作成による同じinode番号の再利用と区別できるようにする。
    let replacement = dir.path().join("replacement");
    std::fs::write(&replacement, b"someone else's file").required()?;
    std::fs::rename(&replacement, &path).required()?;
    let mut warnings = Vec::new();

    let outcome = archive.cleanup_after(Ok(42), &mut warnings, &mut SilentProgress);

    assert_eq!(
        outcome.required_because("cleanup does not change a successful outcome")?,
        42
    );
    assert!(
        warnings.is_empty(),
        "a silently replaced file is not reported as a cleanup failure: {warnings:?}"
    );
    assert!(
        path.exists(),
        "the file that replaced the archive is left in place"
    );
    assert_eq!(
        std::fs::read(&path).required()?,
        b"someone else's file",
        "what replaced the archive is untouched"
    );
    Ok(())
}

#[test]
fn an_unobservable_replacement_is_reported_the_same_way_as_a_removal_failure() -> Checked {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().required()?;
    let sub = dir.path().join("sub");
    std::fs::create_dir(&sub).required()?;
    let path = sub.join("template-000000000000.tar");
    std::fs::write(&path, b"archive").required()?;
    let archive = TransientArchive::new(path.clone()).required()?;
    // 消す権利はfile自身ではなく、それを載せているdirectoryが持つ。実体を確かめ直す
    // こともできなくする。
    std::fs::set_permissions(&sub, std::fs::Permissions::from_mode(0o000)).required()?;

    let mut warnings = Vec::new();
    let outcome = archive.cleanup_after(Ok(42), &mut warnings, &mut SilentProgress);

    std::fs::set_permissions(&sub, std::fs::Permissions::from_mode(0o700)).required()?;

    assert_eq!(
        outcome.required_because("a cleanup failure never hides the value that was produced")?,
        42
    );
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert_eq!(warnings[0].description.id, "warning-archive-cleanup-failed");
    Ok(())
}

#[test]
fn a_path_that_cannot_be_observed_is_never_adopted_as_an_archive() -> Checked {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().required()?;
    let sub = dir.path().join("sub");
    std::fs::create_dir(&sub).required()?;
    let path = sub.join("template-000000000000.tar");
    // 存在を確かめる権利すら無いdirectoryの下に置く。不在と観測不能を混同しない。
    std::fs::set_permissions(&sub, std::fs::Permissions::from_mode(0o000)).required()?;

    let created = TransientArchive::new(path.clone());

    std::fs::set_permissions(&sub, std::fs::Permissions::from_mode(0o700)).required()?;

    let error = created
        .refused_because("a path that cannot be observed is never treated as a fresh archive")?;
    assert_eq!(error.first_id(), Some(ErrorId::ProjectPathUnreadable));
    Ok(())
}

#[test]
fn dropping_an_archive_that_was_never_cleaned_up_removes_it() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let path = dir.path().join("template-000000000000.tar");
    std::fs::write(&path, b"archive").required()?;

    drop(TransientArchive::new(path.clone()).required()?);

    assert!(!path.exists(), "the last resort cleanup runs on drop");
    Ok(())
}
