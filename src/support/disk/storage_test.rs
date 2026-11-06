use super::*;
use crate::testing::host::FakeSbx;
use crate::testing::outcome::{Checked, Refused, Required};

fn df(free: u64) -> String {
    format!(
        "Filesystem 1024-blocks Used Available Capacity Mounted on\noverlay 100 10 {free} 10% /\n"
    )
}

#[test]
fn block_and_inode_exhaustion_are_separate_and_the_probe_never_writes() -> Checked {
    for (blocks, inodes) in [(0, 3), (100, 0), (100, 3)] {
        let host = FakeSbx::listing("[]")
            .answering("exec sandbox -- df -Pk /", 0, &df(blocks))
            .answering(
                "exec sandbox -- df -Pi /",
                0,
                &format!(
                    "Filesystem Inodes IUsed IFree IUse% Mounted on\noverlay 10 7 {inodes} 70% /\n"
                ),
            );
        let storage = Storage::sandbox(&host, "sandbox", &[("storage-sandbox-root", "/")])
            .required()?
            .remove(0);
        assert_eq!(storage.exhausted(), blocks == 0 || inodes == 0);
        if storage.exhausted() {
            storage
                .require_available()
                .refused_because("zero is an operational failure")?;
        } else {
            storage.require_available().required()?;
        }
        assert!(
            host.calls()
                .iter()
                .all(|args| args.iter().any(|arg| arg == "df"))
        );
    }
    Ok(())
}

#[test]
fn failed_and_malformed_probes_retain_the_original_error() -> Checked {
    let host = FakeSbx::listing("[]")
        .answering("exec sandbox -- df -Pk /", 1, "")
        .answering("exec sandbox -- df -Pi /", 0, "bad");
    let storage = Storage::sandbox(&host, "sandbox", &[("storage-sandbox-root", "/")])
        .required()?
        .remove(0);
    let error = storage
        .require_available()
        .refused_because("unknown does not mean available")?;
    assert!(
        error
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.external.is_some())
    );
    assert!(storage.available_kib.is_none());
    for bad in [
        "Filesystem Inodes IUsed IFree IUse% Mounted on\noverlay 1 1 x 99% /",
        "Filesystem Inodes IUsed IFree IUse% Mounted on\noverlay 1 1 0 99% /\nextra",
    ] {
        inode_count(bad).refused_because("inode counters need a single complete row")?;
    }
    Storage::empty("storage-host-metadata", "/unknown".into())
        .require_available()
        .refused_because("missing capacity must not pass a preflight")?;
    Ok(())
}

#[test]
fn several_filesystems_share_one_probe_and_preserve_individual_failures() -> Checked {
    let host = FakeSbx::listing("[]")
        .answering("exec sandbox -- df -Pk /", 0, &df(100))
        .answering(
            "exec sandbox -- df -Pi /",
            0,
            "Filesystem Inodes IUsed IFree IUse% Mounted on\noverlay 10 7 3 70% /\n",
        )
        .answering("exec sandbox -- df -Pk /var/lib/docker", 1, "")
        .answering("exec sandbox -- df -Pi /var/lib/docker", 1, "");
    let storage = Storage::sandbox(
        &host,
        "sandbox",
        &[
            ("storage-sandbox-root", "/"),
            ("storage-sandbox-docker", "/var/lib/docker"),
        ],
    )
    .required()?;
    assert_eq!(host.calls().len(), 1);
    assert_eq!(storage.len(), 2);
    storage[0].require_available().required()?;
    let error = storage[1]
        .require_available()
        .refused_because("Docker data could not be read")?;
    assert!(
        error
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.external.is_some())
    );
    Ok(())
}

struct ProbeFailure {
    canceled: bool,
}

impl crate::testing::host::AnsweredHost for ProbeFailure {
    fn has_command(&self, _: &str) -> bool {
        true
    }
    fn answer(&self, spec: &crate::boundary::host::CommandSpec) -> Result<CommandOutcome> {
        if self.canceled {
            return Err(Error::Canceled);
        }
        Ok(crate::testing::command::outcome_with_stderr(
            spec,
            125,
            "",
            "cannot execute probe\n",
        ))
    }
}

#[test]
fn a_canceled_probe_is_propagated_and_a_failed_batch_remains_unobservable() -> Checked {
    let error = Storage::sandbox(
        &ProbeFailure { canceled: true },
        "sandbox",
        &[("storage-sandbox-root", "/")],
    )
    .refused_because("the user interrupted the capacity check")?;
    assert_eq!(error, Error::Canceled);
    let storage = Storage::sandbox(
        &ProbeFailure { canceled: false },
        "sandbox",
        &[("storage-sandbox-root", "/")],
    )
    .required()?;
    let error = storage[0]
        .require_available()
        .refused_because("the batch could not run")?;
    assert!(
        error.diagnostics()[0]
            .external
            .as_ref()
            .required()?
            .stderr_text()
            .contains("cannot execute probe")
    );
    let mut storage = Storage::empty("storage-sandbox-root", "/".into());
    let error = storage
        .record::<u64>(Err(Error::Canceled))
        .refused_because("an individual cancellation is still cancellation")?;
    assert_eq!(error, Error::Canceled);
    Ok(())
}

#[test]
fn host_observation_uses_an_existing_ancestor_and_reports_unreadable_locations() -> Checked {
    let directory = tempfile::tempdir().required()?;
    let target = directory.path().join("not-created/cache/archive");
    let storage = Storage::host("storage-host-archives", &target);
    storage.require_available().required()?;
    assert!(storage.available_kib.is_some());
    assert!(!target.exists());
    let file = directory.path().join("file");
    std::fs::write(&file, b"file").required()?;
    Storage::host("storage-host-archives", &file.join("child"))
        .require_available()
        .refused_because("a non-directory parent cannot be treated as healthy storage")?;
    Ok(())
}
