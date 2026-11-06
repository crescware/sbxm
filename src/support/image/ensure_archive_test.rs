use std::fs;
use std::path::Path;

use crate::boundary::host::{CommandOutcome, CommandSpec};
use crate::design::{Fact, SilentProgress};
use crate::diagnostics::{Error, ErrorId, Result};
use crate::hash::short_hex;
use crate::paths::{ProjectParent, ProjectPaths};
use crate::testing::command::outcome_with_stderr;
use crate::testing::host::AnsweredHost;
use crate::testing::outcome::{Checked, Refused, Required};
use crate::testing::recorded_output::RecordedOutput;
use crate::testing::value::DIGEST;

use super::*;
use crate::support::image::{expected_labels, fake, image_name};

struct PartialSave {
    code: i32,
}

impl AnsweredHost for PartialSave {
    fn has_command(&self, _: &str) -> bool {
        true
    }
    fn answer(&self, spec: &CommandSpec) -> Result<CommandOutcome> {
        if let Some(index) = spec.args.iter().position(|arg| arg == "--output") {
            fs::write(&spec.args[index + 1], b"partial archive").map_err(|error| {
                paths::atomic_write_failed(Path::new(&spec.args[index + 1]), &error.to_string())
            })?;
            return Ok(outcome_with_stderr(
                spec,
                self.code,
                "",
                "No space left on device",
            ));
        }
        Ok(outcome_with_stderr(spec, 0, "reachable", ""))
    }
}

#[test]
fn save_failures_and_validation_failures_remove_the_partial_archive_immediately() -> Checked {
    for code in [0, 1] {
        let directory = tempfile::tempdir().required()?;
        let paths = ProjectPaths::derive(
            &ProjectParent::at(directory.path()).required()?,
            &fake::canonical()?,
        );
        fs::create_dir_all(paths.cache_dir()).required()?;
        let old = paths.template_archive(short_hex(DIGEST));
        fs::write(&old, b"old archive").required()?;
        let image = BuiltImage {
            name: image_name(&fake::sandbox()?, DIGEST),
            id: "id".into(),
            labels: expected_labels(&fake::canonical()?, DIGEST),
            built: true,
            warnings: vec![],
        };
        let error = ensure_archive(
            &PartialSave { code },
            &paths,
            &image,
            DIGEST,
            &mut SilentProgress,
        )
        .refused_because("the producer or its incomplete archive failed")?;
        assert_eq!(
            error.first_id(),
            Some(if code == 0 {
                ErrorId::ArchiveUnusable
            } else {
                ErrorId::ExternalCommandFailed
            })
        );
        assert!(!paths.template_archive_temp(short_hex(DIGEST)).exists());
        assert_eq!(fs::read(&old).required()?, b"old archive");
    }
    Ok(())
}

#[test]
fn cleanup_keeps_links_and_reports_its_failure_without_replacing_the_original_error() -> Checked {
    let directory = tempfile::tempdir().required()?;
    let missing = directory.path().join("missing");
    remove_partial(&missing).required()?;
    let link = directory.path().join("link");
    std::os::unix::fs::symlink(&missing, &link).required()?;
    remove_partial(&link).refused_because("links are not this run's output")?;
    assert!(fs::symlink_metadata(&link).is_ok());
    let mut progress = RecordedOutput::new();
    let error = paths::atomic_write_failed(&missing, "primary failure");
    let decorated = with_cleanup(
        error.clone(),
        &missing,
        Err(std::io::Error::from_raw_os_error(28)),
        &mut progress,
    );
    assert_eq!(decorated.first_id(), error.first_id());
    assert!(
        decorated.diagnostics()[0]
            .facts
            .contains(&Fact::path(&paths::display(&missing)))
    );
    assert_eq!(
        with_cleanup(
            Error::Canceled,
            &missing,
            Err(std::io::Error::from_raw_os_error(28)),
            &mut progress
        ),
        Error::Canceled
    );
    assert_eq!(progress.warnings.len(), 1);
    assert_eq!(
        with_cleanup(error.clone(), &missing, Ok(()), &mut progress),
        error
    );
    Ok(())
}
