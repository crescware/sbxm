use crate::testing::outcome::{Checked, Required};

use crate::config::ConfigLocation;

pub fn location_with_config(text: Option<&str>) -> Checked<(tempfile::TempDir, ConfigLocation)> {
    let dir = tempfile::tempdir().required_because("temporary home")?;
    let location = ConfigLocation::from_home(dir.path().to_path_buf());
    if let Some(text) = text {
        use std::os::unix::fs::PermissionsExt;
        std::fs::create_dir_all(location.dir()).required()?;
        std::fs::set_permissions(location.dir(), std::fs::Permissions::from_mode(0o700))
            .required()?;
        std::fs::write(location.config_file(), text).required()?;
        std::fs::set_permissions(
            location.config_file(),
            std::fs::Permissions::from_mode(0o600),
        )
        .required()?;
    }
    Ok((dir, location))
}
