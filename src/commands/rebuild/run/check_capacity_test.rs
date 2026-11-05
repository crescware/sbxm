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
