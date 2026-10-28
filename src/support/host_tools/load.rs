use std::fs::File;
use std::io::Read;
use std::path::Path;

use crate::paths::{self, PRIVATE_FILE_MODE, PathScope};

use super::{Loaded, parse};

/// 記録fileを読む。
///
/// 記録は確認を省略する根拠であるため、configと同じ要件（本人所有、`0600`、symlinkでは
/// ない通常file）を課す。開いたfileのmetadataで確かめ、確かめたfileをそのまま読む。
pub(super) fn load(path: &Path) -> Loaded {
    if paths::is_symlink(path) {
        return Loaded::Unusable;
    }
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Loaded::Missing,
        Err(_) => return Loaded::Corrupt,
    };
    if paths::require_private_file(&file, path, PRIVATE_FILE_MODE, PathScope::ConfigFile).is_err() {
        return Loaded::Unusable;
    }
    let mut text = String::new();
    if file.read_to_string(&mut text).is_err() {
        return Loaded::Corrupt;
    }
    match parse(&text) {
        Some(records) => Loaded::Records(records),
        None => Loaded::Corrupt,
    }
}
