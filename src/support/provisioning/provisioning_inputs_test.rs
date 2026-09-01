use std::fs;

use crate::config::GlobalConfig;
use crate::diagnostics::ErrorId;
use crate::i18n::Locale;
use crate::testing::outcome::{Checked, Refused, Required};
use crate::testing::repository::project_paths;

use super::ProvisioningInputs;

fn config() -> GlobalConfig {
    GlobalConfig {
        language: Some(Locale::En),
        git_identity: None,
        files: Vec::new(),
    }
}

#[test]
fn an_absent_dockerfile_is_refused_rather_than_silently_captured() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let paths = project_paths(dir.path())?;

    let error = ProvisioningInputs::capture(&paths, &config(), None)
        .refused_because("a snapshot cannot be captured from a Dockerfile that is not there")?;
    assert_eq!(error.first_id(), Some(ErrorId::ProjectPathUnreadable));
    Ok(())
}

#[test]
fn a_snapshot_removed_after_capture_fails_verification() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let paths = project_paths(dir.path())?;
    fs::write(paths.dockerfile(), b"FROM example\n").required_because("write the Dockerfile")?;

    let inputs =
        ProvisioningInputs::capture(&paths, &config(), None).required_because("capture")?;
    fs::remove_file(&inputs.dockerfile_path).required_because("remove the snapshot")?;

    let error = inputs
        .verify_unchanged()
        .refused_because("a snapshot that disappeared cannot be verified unchanged")?;
    assert_eq!(
        error.first_id(),
        Some(ErrorId::InitialProvisioningSnapshotChanged)
    );
    Ok(())
}

#[test]
fn a_snapshot_edited_after_capture_fails_verification() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let paths = project_paths(dir.path())?;
    fs::write(paths.dockerfile(), b"FROM example\n").required_because("write the Dockerfile")?;

    let inputs =
        ProvisioningInputs::capture(&paths, &config(), None).required_because("capture")?;
    fs::write(&inputs.dockerfile_path, b"FROM tampered\n")
        .required_because("tamper with the snapshot")?;

    let error = inputs
        .verify_unchanged()
        .refused_because("a snapshot whose bytes changed is not the one that was captured")?;
    assert_eq!(
        error.first_id(),
        Some(ErrorId::InitialProvisioningSnapshotChanged)
    );
    Ok(())
}

#[test]
fn a_target_generation_matching_the_live_dockerfile_still_writes_a_snapshot() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let paths = project_paths(dir.path())?;
    fs::write(paths.dockerfile(), b"FROM example\n").required_because("write the Dockerfile")?;

    let inputs =
        ProvisioningInputs::capture(&paths, &config(), None).required_because("capture")?;
    let retargeted =
        ProvisioningInputs::capture(&paths, &config(), Some(inputs.dockerfile_sha256.as_str()))
            .required_because("a target equal to the live Dockerfile still captures a snapshot")?;
    retargeted
        .verify_unchanged()
        .required_because("the snapshot it just wrote verifies as unchanged")?;
    Ok(())
}

#[test]
fn a_snapshot_directory_that_cannot_be_written_to_is_refused() -> Checked {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().required()?;
    let paths = project_paths(dir.path())?;
    fs::write(paths.dockerfile(), b"FROM example\n").required_because("write the Dockerfile")?;
    fs::create_dir_all(paths.snapshot_dir()).required_because("create the snapshot directory")?;
    fs::set_permissions(paths.snapshot_dir(), fs::Permissions::from_mode(0o500))
        .required_because("make the snapshot directory read-only")?;

    let error = ProvisioningInputs::capture(&paths, &config(), None)
        .refused_because("a snapshot cannot be written into a directory that refuses writes")?;
    assert_eq!(error.first_id(), Some(ErrorId::AtomicWriteFailed));

    fs::set_permissions(paths.snapshot_dir(), fs::Permissions::from_mode(0o700))
        .required_because("restore permissions so the temp directory can be cleaned up")?;
    Ok(())
}

#[test]
fn a_target_generation_that_differs_from_the_live_dockerfile_skips_the_snapshot() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let paths = project_paths(dir.path())?;
    fs::write(paths.dockerfile(), b"FROM example\n").required_because("write the Dockerfile")?;

    let inputs = ProvisioningInputs::capture(&paths, &config(), Some("a-different-generation"))
        .required_because("a stale target does not need the current Dockerfile's bytes")?;
    assert_eq!(inputs.dockerfile_sha256, "a-different-generation");
    assert!(
        !inputs.dockerfile_path.exists(),
        "no snapshot is written for a generation the live Dockerfile does not represent"
    );
    inputs
        .verify_unchanged()
        .required_because("verification skips a snapshot that was never written")?;
    Ok(())
}
