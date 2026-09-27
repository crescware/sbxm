use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use crate::config::{FileDeclaration, GlobalConfig, HostFileSource, SandboxHomeRelativePath};
use crate::diagnostics::ErrorId;
use crate::hash::sha256_hex;
use crate::i18n::Locale;
use crate::metadata::{InitialProvisioningFile, InitialProvisioningIntent};
use crate::paths::{self, ProjectPaths};
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

/// `source`を1件だけ宣言したconfig。
fn declaring(source: &Path) -> Checked<GlobalConfig> {
    Ok(GlobalConfig {
        files: vec![FileDeclaration {
            source: HostFileSource::new(&paths::display(source))
                .required_because("valid source")?,
            destination: SandboxHomeRelativePath::new(".config/example.yaml")
                .required_because("valid destination")?,
        }],
        ..config()
    })
}

/// 内容別blobの置き場を、owner以外に開かないmodeで先に作っておく。
fn blob_directory(paths: &ProjectPaths, mode: u32) -> Checked {
    let directory = paths.snapshot_dir().join("sha256");
    fs::create_dir_all(&directory).required_because("create the blob directory")?;
    fs::set_permissions(paths.snapshot_dir(), fs::Permissions::from_mode(0o700))
        .required_because("keep the snapshot directory private")?;
    fs::set_permissions(&directory, fs::Permissions::from_mode(mode))
        .required_because("set the blob directory mode")?;
    Ok(())
}

/// 期待digestの名前で、別のbyte列を持つprivateなblobを置く。
fn impostor_blob(paths: &ProjectPaths, digest: &str) -> Checked {
    blob_directory(paths, 0o700)?;
    let blob = paths.snapshot_blob(digest);
    fs::write(&blob, b"other bytes\n").required_because("write the impostor blob")?;
    fs::set_permissions(&blob, fs::Permissions::from_mode(0o600))
        .required_because("the impostor is as private as a real blob")?;
    Ok(())
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

#[test]
fn a_snapshot_directory_opened_up_to_others_is_refused_before_anything_is_copied_into_it() -> Checked
{
    let dir = tempfile::tempdir().required()?;
    let paths = project_paths(dir.path())?;
    fs::write(paths.dockerfile(), b"FROM example\n").required_because("write the Dockerfile")?;
    fs::create_dir_all(paths.snapshot_dir()).required_because("create the snapshot directory")?;
    fs::set_permissions(paths.snapshot_dir(), fs::Permissions::from_mode(0o755))
        .required_because("open the snapshot directory up to group and other")?;

    let error = ProvisioningInputs::capture(&paths, &config(), None)
        .refused_because("others could swap a snapshot inside a directory they can enter")?;
    assert_eq!(
        error.first_id(),
        Some(ErrorId::ProjectFilePermissionTooOpen)
    );
    assert!(
        !paths.snapshot_dir().join("sha256").exists(),
        "no blob is written into the open directory"
    );
    Ok(())
}

#[test]
fn a_dockerfile_replaced_by_a_symlink_is_refused_rather_than_followed() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let paths = project_paths(dir.path())?;
    let elsewhere = dir.path().join("elsewhere");
    fs::write(&elsewhere, b"FROM elsewhere\n").required_because("write the link target")?;
    std::os::unix::fs::symlink(&elsewhere, paths.dockerfile()).required()?;

    let error = ProvisioningInputs::capture(&paths, &config(), None)
        .refused_because("the bytes behind a symlink are not the project's Dockerfile")?;
    assert_eq!(error.first_id(), Some(ErrorId::ProjectPathSymlink));
    assert!(
        !paths.snapshot_dir().join("sha256").exists(),
        "nothing is captured from the link target"
    );
    Ok(())
}

#[test]
fn a_blob_already_holding_other_bytes_under_the_declared_files_digest_is_refused() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let paths = project_paths(dir.path())?;
    fs::write(paths.dockerfile(), b"FROM example\n").required_because("write the Dockerfile")?;
    let source = dir.path().join("declared.yaml");
    fs::write(&source, b"declared = true\n").required_because("write the declared file")?;
    let digest = sha256_hex(b"declared = true\n");
    impostor_blob(&paths, &digest)?;

    let error = ProvisioningInputs::capture(&paths, &declaring(&source)?, None).refused_because(
        "a blob is named by its content, so other bytes under the name are not it",
    )?;
    assert_eq!(
        error.first_id(),
        Some(ErrorId::InitialProvisioningSnapshotChanged)
    );
    assert_eq!(
        fs::read(paths.snapshot_blob(&digest)).required()?,
        b"other bytes\n",
        "the existing blob is neither trusted nor overwritten"
    );
    Ok(())
}

#[test]
fn a_declared_file_snapshot_edited_after_capture_fails_verification() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let paths = project_paths(dir.path())?;
    fs::write(paths.dockerfile(), b"FROM example\n").required_because("write the Dockerfile")?;
    let source = dir.path().join("declared.yaml");
    fs::write(&source, b"declared = true\n").required_because("write the declared file")?;

    let inputs = ProvisioningInputs::capture(&paths, &declaring(&source)?, None)
        .required_because("capture")?;
    fs::write(
        inputs.files[0].declaration.source.as_path(),
        b"declared = tampered\n",
    )
    .required_because("tamper with the declared file's snapshot")?;

    let error = inputs
        .verify_unchanged()
        .refused_because("the declared file about to be copied is not the one that was captured")?;
    assert_eq!(
        error.first_id(),
        Some(ErrorId::InitialProvisioningSnapshotChanged)
    );
    Ok(())
}

#[test]
fn a_snapshot_opened_up_to_others_is_refused_before_its_bytes_are_trusted() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let paths = project_paths(dir.path())?;
    fs::write(paths.dockerfile(), b"FROM example\n").required_because("write the Dockerfile")?;

    let inputs =
        ProvisioningInputs::capture(&paths, &config(), None).required_because("capture")?;
    fs::set_permissions(&inputs.dockerfile_path, fs::Permissions::from_mode(0o644))
        .required_because("open the snapshot up to group and other")?;

    let error = inputs
        .verify_unchanged()
        .refused_because("a snapshot others can read may also have been swapped by them")?;
    assert_eq!(
        error.first_id(),
        Some(ErrorId::ProjectFilePermissionTooOpen)
    );
    Ok(())
}

#[test]
fn a_dockerfile_blob_holding_other_bytes_is_refused_whether_or_not_the_build_needs_it() -> Checked {
    for require_dockerfile in [false, true] {
        let dir = tempfile::tempdir().required()?;
        let paths = project_paths(dir.path())?;
        let digest = sha256_hex(b"FROM recorded\n");
        impostor_blob(&paths, &digest)?;
        let intent = InitialProvisioningIntent {
            target_dockerfile_sha256: digest,
            files: Vec::new(),
        };

        let error = ProvisioningInputs::resume(&paths, &intent, require_dockerfile)
            .refused_because("a blob that does not hold its digest is not the recorded input")?;
        assert_eq!(
            error.first_id(),
            Some(ErrorId::InitialProvisioningSnapshotChanged),
            "require_dockerfile = {require_dockerfile}"
        );
    }
    Ok(())
}

#[test]
fn a_recorded_baseline_that_cannot_be_recovered_is_refused() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let paths = project_paths(dir.path())?;
    let recorded = [InitialProvisioningFile {
        source: dir.path().join("removed-source").display().to_string(),
        destination: ".config/example.yaml".to_string(),
        sha256: sha256_hex(b"declared = true\n"),
    }];

    let error = ProvisioningInputs::from_recorded_files(&paths, &recorded)
        .refused_because("neither a blob, a legacy snapshot nor the source holds the bytes")?;
    assert_eq!(
        error.first_id(),
        Some(ErrorId::InitialProvisioningSnapshotChanged)
    );
    Ok(())
}

#[test]
fn a_recorded_file_is_recovered_from_its_source_when_no_snapshot_is_left() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let paths = project_paths(dir.path())?;
    let declared = b"declared = true\n";
    let source = dir.path().join("declared.yaml");
    fs::write(&source, declared).required_because("the source still holds the bytes")?;
    let intent = InitialProvisioningIntent {
        target_dockerfile_sha256: "completed-generation".to_string(),
        files: vec![InitialProvisioningFile {
            source: paths::display(&source),
            destination: ".config/example.yaml".to_string(),
            sha256: sha256_hex(declared),
        }],
    };

    let inputs = ProvisioningInputs::resume(&paths, &intent, false)
        .required_because("the source whose digest matches is imported")?;
    inputs
        .verify_unchanged()
        .required_because("the imported blob verifies")?;
    assert_eq!(
        fs::read(paths.snapshot_blob(&sha256_hex(declared))).required()?,
        declared
    );
    assert_eq!(inputs.files[0].original_source, paths::display(&source));
    Ok(())
}

#[test]
fn a_recovered_input_that_cannot_be_stored_as_a_blob_is_refused() -> Checked {
    for require_dockerfile in [false, true] {
        let dir = tempfile::tempdir().required()?;
        let paths = project_paths(dir.path())?;
        let dockerfile = b"FROM legacy\n";
        blob_directory(&paths, 0o500)?;
        fs::write(paths.snapshot_dockerfile(), dockerfile).required_because("legacy Dockerfile")?;
        let intent = InitialProvisioningIntent {
            target_dockerfile_sha256: sha256_hex(dockerfile),
            files: Vec::new(),
        };

        let result = ProvisioningInputs::resume(&paths, &intent, require_dockerfile);
        fs::set_permissions(
            paths.snapshot_dir().join("sha256"),
            fs::Permissions::from_mode(0o700),
        )
        .required_because("restore permissions so the temp directory can be cleaned up")?;

        let error = result.refused_because("the recovered bytes have nowhere to be fixed")?;
        assert_eq!(
            error.first_id(),
            Some(ErrorId::AtomicWriteFailed),
            "require_dockerfile = {require_dockerfile}"
        );
        assert!(!paths.snapshot_blob(&sha256_hex(dockerfile)).exists());
    }
    Ok(())
}

#[test]
fn a_blob_name_taken_by_a_dangling_symlink_is_refused_rather_than_followed() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let paths = project_paths(dir.path())?;
    let dockerfile = b"FROM example\n";
    fs::write(paths.dockerfile(), dockerfile).required_because("write the Dockerfile")?;
    blob_directory(&paths, 0o700)?;
    let dangling = dir.path().join("nowhere");
    let blob = paths.snapshot_blob(&sha256_hex(dockerfile));
    std::os::unix::fs::symlink(&dangling, &blob).required()?;

    let error = ProvisioningInputs::capture(&paths, &config(), None)
        .refused_because("the name a blob needs is already taken")?;
    assert_eq!(error.first_id(), Some(ErrorId::AtomicWriteFailed));
    assert!(!dangling.exists(), "nothing is written through the symlink");
    assert!(
        fs::symlink_metadata(&blob)
            .required()?
            .file_type()
            .is_symlink(),
        "the symlink is left as it was"
    );
    Ok(())
}

#[test]
fn a_blob_directory_that_cannot_be_listed_still_receives_the_snapshot() -> Checked {
    // blobを置いたあとのdirectoryのsyncは、できれば行う後始末である。書き込めるが
    // 開けないdirectoryでも、snapshot自体は置ける。
    let dir = tempfile::tempdir().required()?;
    let paths = project_paths(dir.path())?;
    let dockerfile = b"FROM example\n";
    fs::write(paths.dockerfile(), dockerfile).required_because("write the Dockerfile")?;
    blob_directory(&paths, 0o300)?;

    let result = ProvisioningInputs::capture(&paths, &config(), None);
    fs::set_permissions(
        paths.snapshot_dir().join("sha256"),
        fs::Permissions::from_mode(0o700),
    )
    .required_because("restore permissions so the temp directory can be cleaned up")?;

    let inputs = result.required_because("the blob is written without syncing its directory")?;
    inputs
        .verify_unchanged()
        .required_because("the written blob verifies")?;
    assert_eq!(
        fs::read(paths.snapshot_blob(&sha256_hex(dockerfile))).required()?,
        dockerfile
    );
    Ok(())
}
