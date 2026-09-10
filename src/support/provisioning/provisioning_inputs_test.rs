use std::fs;

use crate::config::GlobalConfig;
use crate::diagnostics::ErrorId;
use crate::hash::sha256_hex;
use crate::i18n::Locale;
use crate::metadata::{InitialProvisioningFile, InitialProvisioningIntent};
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
fn a_dockerfile_that_cannot_be_read_is_refused() -> Checked {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().required()?;
    let paths = project_paths(dir.path())?;
    fs::write(paths.dockerfile(), b"FROM example\n").required_because("write the Dockerfile")?;
    fs::set_permissions(paths.dockerfile(), fs::Permissions::from_mode(0o000))
        .required_because("make the Dockerfile unreadable")?;

    let error = ProvisioningInputs::capture(&paths, &config(), None)
        .refused_because("a Dockerfile that exists but cannot be read cannot be captured")?;
    assert_eq!(error.first_id(), Some(ErrorId::ProjectPathUnreadable));

    fs::set_permissions(paths.dockerfile(), fs::Permissions::from_mode(0o644))
        .required_because("restore permissions so the temp directory can be cleaned up")?;
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

#[test]
fn legacy_snapshots_are_migrated_to_verified_content_blobs() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let paths = project_paths(dir.path())?;
    fs::create_dir_all(paths.snapshot_dir())
        .required_because("create legacy snapshot directory")?;
    let dockerfile = b"FROM legacy\n";
    let declared = b"declared = true\n";
    fs::write(paths.snapshot_dockerfile(), dockerfile).required_because("legacy Dockerfile")?;
    fs::write(paths.snapshot_file(0), declared).required_because("legacy declared file")?;
    let intent = InitialProvisioningIntent {
        target_dockerfile_sha256: sha256_hex(dockerfile),
        files: vec![InitialProvisioningFile {
            source: dir.path().join("removed-source").display().to_string(),
            destination: ".config/example.yaml".to_string(),
            sha256: sha256_hex(declared),
        }],
    };

    let inputs = ProvisioningInputs::resume(&paths, &intent, true)
        .required_because("resume from legacy snapshots")?;
    inputs
        .verify_unchanged()
        .required_because("verify migrated inputs")?;
    assert_eq!(
        fs::read(paths.snapshot_blob(&sha256_hex(dockerfile))).required()?,
        dockerfile
    );
    assert_eq!(
        fs::read(paths.snapshot_blob(&sha256_hex(declared))).required()?,
        declared
    );

    fs::remove_file(paths.snapshot_dockerfile()).required()?;
    fs::remove_file(paths.snapshot_file(0)).required()?;
    ProvisioningInputs::resume(&paths, &intent, true)
        .required_because("the migrated blobs resume without legacy files")?;
    Ok(())
}

#[test]
fn a_completed_generation_does_not_require_missing_dockerfile_bytes() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let paths = project_paths(dir.path())?;
    let intent = InitialProvisioningIntent {
        target_dockerfile_sha256: "recorded-generation".to_string(),
        files: Vec::new(),
    };

    let inputs = ProvisioningInputs::resume(&paths, &intent, false)
        .required_because("existing artifacts make the Dockerfile bytes optional")?;
    assert!(!inputs.dockerfile_path.exists());
    inputs
        .verify_unchanged()
        .required_because("there is no Dockerfile snapshot to verify")?;
    Ok(())
}

#[test]
fn an_optional_legacy_dockerfile_is_imported_when_it_is_still_available() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let paths = project_paths(dir.path())?;
    fs::create_dir_all(paths.snapshot_dir())
        .required_because("create legacy snapshot directory")?;
    let dockerfile = b"FROM legacy\n";
    fs::write(paths.snapshot_dockerfile(), dockerfile).required_because("legacy Dockerfile")?;
    let intent = InitialProvisioningIntent {
        target_dockerfile_sha256: sha256_hex(dockerfile),
        files: Vec::new(),
    };

    let inputs = ProvisioningInputs::resume(&paths, &intent, false)
        .required_because("optional available bytes are imported")?;
    inputs
        .verify_unchanged()
        .required_because("verify imported Dockerfile")?;
    assert!(paths.snapshot_blob(&sha256_hex(dockerfile)).exists());
    Ok(())
}

#[test]
fn an_invalid_recorded_destination_is_refused_during_resume() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let paths = project_paths(dir.path())?;
    fs::create_dir_all(paths.snapshot_dir())
        .required_because("create legacy snapshot directory")?;
    let declared = b"declared = true\n";
    fs::write(paths.snapshot_file(0), declared).required_because("legacy declared file")?;
    let intent = InitialProvisioningIntent {
        target_dockerfile_sha256: "completed-generation".to_string(),
        files: vec![InitialProvisioningFile {
            source: dir.path().join("removed-source").display().to_string(),
            destination: "/absolute/path".to_string(),
            sha256: sha256_hex(declared),
        }],
    };

    let error = ProvisioningInputs::resume(&paths, &intent, false)
        .refused_because("recorded destinations remain sandbox-home relative")?;
    assert_eq!(
        error.first_id(),
        Some(ErrorId::InitialProvisioningSnapshotChanged)
    );
    Ok(())
}

#[test]
fn a_required_recorded_input_that_cannot_be_recovered_is_refused() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let paths = project_paths(dir.path())?;
    let intent = InitialProvisioningIntent {
        target_dockerfile_sha256: "missing-generation".to_string(),
        files: Vec::new(),
    };

    let error = ProvisioningInputs::resume(&paths, &intent, true)
        .refused_because("a build cannot proceed without its recorded Dockerfile bytes")?;
    assert_eq!(
        error.first_id(),
        Some(ErrorId::InitialProvisioningSnapshotChanged)
    );
    Ok(())
}
