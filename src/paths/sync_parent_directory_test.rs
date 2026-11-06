use super::*;
use crate::testing::outcome::{Checked, Refused, Required};

#[test]
fn delayed_disk_errors_are_reported_without_removing_the_committed_file() -> Checked {
    let directory = tempfile::tempdir().required()?;
    let target = directory.path().join("committed");
    std::fs::write(&target, b"new content").required()?;
    sync_parent_directory(&target).required()?;
    for code in [28, 5, 13] {
        let error = checked(&target, Err(std::io::Error::from_raw_os_error(code)))
            .refused_because("a successful rename is not a successful directory sync")?;
        assert_eq!(
            error.first_id(),
            Some(ErrorId::AtomicWriteDurabilityUnconfirmed)
        );
        assert_eq!(std::fs::read(&target).required()?, b"new content");
    }
    sync_parent_directory(&target.join("missing/child"))
        .refused_because("an unopenable parent must not be ignored")?;
    Ok(())
}
