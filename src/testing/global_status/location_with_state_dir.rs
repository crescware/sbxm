use std::os::unix::fs::PermissionsExt;

use crate::config::ConfigLocation;
use crate::testing::outcome::{Checked, Required};

/// `~/.sbxm`だけがある一時的なhome。configもregistryも無い。
pub fn location_with_state_dir() -> Checked<(tempfile::TempDir, ConfigLocation)> {
    let dir = tempfile::tempdir().required_because("temporary home")?;
    let location = ConfigLocation::from_home(dir.path().to_path_buf());
    std::fs::create_dir_all(location.dir()).required()?;
    std::fs::set_permissions(location.dir(), std::fs::Permissions::from_mode(0o700)).required()?;
    Ok((dir, location))
}
