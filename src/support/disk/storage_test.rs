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
        let storage = Storage::sandbox(&host, "sandbox", "storage-sandbox-root", "/");
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
    let storage = Storage::sandbox(&host, "sandbox", "storage-sandbox-root", "/");
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
