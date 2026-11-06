use super::*;
use crate::testing::outcome::{Checked, Refused, Required};

#[test]
fn insufficient_positive_space_and_inode_counts_are_rejected() -> Checked {
    let mut storage = Storage::host("storage-host-metadata", std::path::Path::new("/tmp"));
    storage.available_kib = Some(100);
    storage.available_inodes = Some(10);
    require(&storage, 101, 1).refused_because("nonzero space can still be too small")?;
    require(&storage, 1, 11).refused_because("inode capacity is independent of bytes")?;
    require(&storage, 100, 10).required()?;
    storage.available_inodes = None;
    let error = require(&storage, 101, 1)
        .refused_because("an unavailable inode count must not be invented as zero")?;
    assert!(
        error.diagnostics()[0]
            .description
            .args
            .contains(&("free_inodes", "not-observed".to_string()))
    );
    Ok(())
}

#[test]
fn the_target_allocation_unit_must_be_observed_as_a_positive_integer() -> Checked {
    let command = "exec probe -- stat -f -c %S /";
    for text in ["", "0\n", "-1\n", "4096 8192\n"] {
        let host = crate::testing::host::FakeSbx::listing("[]").answering(command, 0, text);
        allocation_unit(&host, "probe").refused_because("allocation unit is not known")?;
    }
    let host = crate::testing::host::FakeSbx::listing("[]").answering(command, 0, "65536\n");
    assert_eq!(allocation_unit(&host, "probe").required()?, 65536);
    let host = crate::testing::host::FakeSbx::listing("[]").answering(command, 1, "");
    allocation_unit(&host, "probe").refused_because("the filesystem query failed")?;
    Ok(())
}
