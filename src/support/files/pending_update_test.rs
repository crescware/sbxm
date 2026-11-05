use std::fs;
use std::os::unix::fs::PermissionsExt;

use crate::hash::sha256_hex;
use crate::testing::host::FakeSbx;
use crate::testing::outcome::{Checked, Refused, Required};
use crate::testing::project::Fixture;

use super::*;

fn record(source: &Path, bytes: &[u8]) -> InitialProvisioningFile {
    InitialProvisioningFile {
        source: paths::display(source),
        destination: ".config/example".into(),
        sha256: sha256_hex(bytes),
    }
}

#[test]
fn a_failed_metadata_write_keeps_the_intent_until_the_new_content_is_recorded() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let source = fixture.workspace_root.join("settings");
    fs::write(&source, b"new contents").required()?;
    let file = record(&source, b"new contents");
    PendingUpdate::begin(&project.paths, &file, Some(sha256_hex(b"old")), true).required()?;
    let before = fs::read(project.paths.metadata_file()).required()?;
    let mut metadata = project.metadata.clone();
    PendingUpdate::recover_with(None, "", &project.paths, &mut metadata, |paths, _| {
        Err(paths::atomic_write_failed(
            &paths.metadata_file(),
            &std::io::Error::from_raw_os_error(28).to_string(),
        ))
    })
    .refused_because("the host became full after the replacement")?;
    assert!(PendingUpdate::load(&project.paths).required()?.is_some());
    assert_eq!(fs::read(project.paths.metadata_file()).required()?, before);
    // 次回はmemory上の更新済みmetadataを流用せず、disk上の正本から再開する。
    let mut metadata = metadata::load(&project.paths).required()?.required()?;
    PendingUpdate::recover_host(&project.paths, &mut metadata).required()?;
    assert!(metadata.declared_files.as_ref().required()?.contains(&file));
    assert!(PendingUpdate::load(&project.paths).required()?.is_none());
    Ok(())
}

#[test]
fn an_update_that_never_reached_the_target_is_discarded_without_rewriting_metadata() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let source = fixture.workspace_root.join("settings");
    fs::write(&source, b"old").required()?;
    PendingUpdate::begin(
        &project.paths,
        &record(&source, b"new"),
        Some(sha256_hex(b"old")),
        true,
    )
    .required()?;
    let mut metadata = project.metadata;
    PendingUpdate::recover_with(None, "", &project.paths, &mut metadata, |_, _| {
        Err(Error::Canceled)
    })
    .required()?;
    assert!(PendingUpdate::load(&project.paths).required()?.is_none());
    assert_eq!(fs::read(&source).required()?, b"old");
    Ok(())
}

#[test]
fn a_sandbox_replacement_is_recorded_even_when_the_host_source_was_edited_again() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let source = fixture.workspace_root.join("settings");
    fs::write(&source, b"edited again").required()?;
    let file = record(&source, b"new in sandbox");
    PendingUpdate::begin(&project.paths, &file, Some(sha256_hex(b"old")), false).required()?;
    PendingUpdate::require_clean(&project.paths)
        .refused_because("other provisioning must not reuse the stale baseline")?;
    let target = "/home/agent/.config/example";
    let host = FakeSbx::listing("[]")
        .answering("exec sandbox -- test -h /home/agent/.config", 1, "")
        .answering("exec sandbox -- test -h /home/agent/.config/example", 1, "")
        .answering(
            &format!("exec sandbox -- sha256sum {target}"),
            0,
            &format!("{}  {target}\n", file.sha256),
        );
    let mut metadata = project.metadata;
    PendingUpdate::recover(&host, "sandbox", &project.paths, &mut metadata).required()?;
    assert!(metadata.declared_files.as_ref().required()?.contains(&file));
    assert_eq!(fs::read(&source).required()?, b"edited again");
    Ok(())
}

#[test]
fn unexpected_target_content_and_invalid_records_are_preserved_for_review() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let source = fixture.workspace_root.join("settings");
    fs::write(&source, b"third party change").required()?;
    PendingUpdate::begin(
        &project.paths,
        &record(&source, b"new"),
        Some(sha256_hex(b"old")),
        true,
    )
    .required()?;
    let mut metadata = project.metadata;
    PendingUpdate::recover_host(&project.paths, &mut metadata)
        .refused_because("the operation cannot claim the new content")?;
    assert_eq!(fs::read(&source).required()?, b"third party change");
    let journal = PendingUpdate::path(&project.paths);
    let valid = fs::read_to_string(&journal).required()?;
    for invalid in [
        "bad yaml: [".into(),
        valid.replace("version: 1", "version: 2"),
        "x".repeat(65_537),
        valid.replace(&file_digest(&valid)?, "bad"),
    ] {
        fs::write(&journal, invalid).required()?;
        PendingUpdate::require_clean(&project.paths)
            .refused_because("invalid data must not be ignored")?;
    }
    fs::write(&journal, valid).required()?;
    fs::set_permissions(&journal, fs::Permissions::from_mode(0o644)).required()?;
    PendingUpdate::load(&project.paths).refused_because("the record must be private")?;
    Ok(())
}

fn file_digest(text: &str) -> Checked<String> {
    let record: PendingUpdate = yaml_serde::from_str(text).required()?;
    Ok(record.sha256)
}

#[test]
fn absent_targets_and_unavailable_sandbox_observation_do_not_invent_a_baseline() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let source = fixture.workspace_root.join("not-created");
    let file = record(&source, b"new");
    PendingUpdate::begin(&project.paths, &file, None, true).required()?;
    let mut metadata = project.metadata;
    PendingUpdate::recover_host(&project.paths, &mut metadata).required()?;
    assert!(metadata.declared_files.is_none());
    PendingUpdate::begin(&project.paths, &file, None, false).required()?;
    PendingUpdate::recover_host(&project.paths, &mut metadata)
        .refused_because("a sandbox update needs a sandbox observation")?;
    assert!(PendingUpdate::load(&project.paths).required()?.is_some());
    let mut invalid = PendingUpdate::load(&project.paths).required()?.required()?;
    invalid.source = "relative".into();
    invalid
        .validate(&project.paths)
        .refused_because("source must be absolute")?;
    invalid.source = file.source;
    invalid.destination = "../escape".into();
    invalid
        .validate(&project.paths)
        .refused_because("destination must remain in agent home")?;
    Ok(())
}
