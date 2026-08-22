use std::fs;
use std::os::unix::fs::PermissionsExt;

use crate::i18n::Locale;
use crate::paths::{PRIVATE_DIR_MODE, PRIVATE_FILE_MODE};
use crate::testing::outcome::{Checked, Required};

use super::{ConfigLocation, observe_at};

fn new_location() -> Checked<(tempfile::TempDir, ConfigLocation)> {
    let home = tempfile::tempdir().required_because("temporary home")?;
    let location = ConfigLocation::from_home(home.path().to_path_buf());
    Ok((home, location))
}

fn write_config(location: &ConfigLocation, text: &[u8]) -> Checked {
    fs::create_dir_all(location.dir()).required_because("create config directory")?;
    fs::set_permissions(location.dir(), fs::Permissions::from_mode(PRIVATE_DIR_MODE))
        .required_because("make config directory private")?;
    fs::write(location.config_file(), text).required_because("write config")?;
    fs::set_permissions(
        location.config_file(),
        fs::Permissions::from_mode(PRIVATE_FILE_MODE),
    )
    .required_because("make config private")?;
    Ok(())
}

#[test]
fn a_valid_configuration_exposes_only_its_saved_language_and_location() -> Checked {
    let (_home, location) = new_location()?;
    write_config(&location, b"version: 1\nlanguage: ja\n")?;

    let observation = observe_at(location.clone());
    assert_eq!(observation.language(), Some(Locale::Ja));
    assert_eq!(observation.location().config_file(), location.config_file());
    Ok(())
}

#[test]
fn a_missing_configuration_falls_back_without_an_error() -> Checked {
    let (_home, location) = new_location()?;
    let observation = observe_at(location);

    assert_eq!(observation.language(), None);
    Ok(())
}

#[test]
fn every_unreadable_configuration_falls_back_to_no_saved_language() -> Checked {
    let cases: &[&[u8]] = &[
        b"version: 1\nlanguage: \"ja\n",
        b"version: 99\n",
        b"version: 1\nlanguage: \xff\n",
    ];
    for contents in cases {
        let (_home, location) = new_location()?;
        write_config(&location, contents)?;
        assert_eq!(observe_at(location).language(), None);
    }

    let (_home, location) = new_location()?;
    write_config(&location, b"version: 1\nlanguage: ja\n")?;
    fs::set_permissions(location.config_file(), fs::Permissions::from_mode(0o644))
        .required_because("make config too open")?;
    assert_eq!(observe_at(location).language(), None);

    let (_home, location) = new_location()?;
    fs::create_dir_all(location.dir()).required_because("create config directory")?;
    fs::set_permissions(location.dir(), fs::Permissions::from_mode(PRIVATE_DIR_MODE))
        .required_because("make config directory private")?;
    fs::create_dir(location.config_file()).required_because("make config path a directory")?;
    assert_eq!(observe_at(location).language(), None);

    let (_home, location) = new_location()?;
    let target = location.dir().join("target.yaml");
    fs::create_dir_all(location.dir()).required_because("create config directory")?;
    fs::write(&target, b"version: 1\nlanguage: ja\n").required_because("write target")?;
    std::os::unix::fs::symlink(&target, location.config_file())
        .required_because("create config symlink")?;
    assert_eq!(observe_at(location).language(), None);
    Ok(())
}
