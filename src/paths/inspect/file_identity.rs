use std::fs::{self, File};
use std::os::unix::fs::MetadataExt;
use std::path::Path;

/// open済みfileと、pathが指す実体が同一かを判定するためのidentity。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileIdentity {
    pub device: u64,
    pub inode: u64,
}

impl FileIdentity {
    pub fn of_open_file(file: &File) -> std::io::Result<FileIdentity> {
        let metadata = file.metadata()?;
        Ok(FileIdentity {
            device: metadata.dev(),
            inode: metadata.ino(),
        })
    }

    /// symlinkを追跡せずpathのidentityを取る。
    pub fn of_path_without_following(path: &Path) -> std::io::Result<FileIdentity> {
        let metadata = fs::symlink_metadata(path)?;
        Ok(FileIdentity {
            device: metadata.dev(),
            inode: metadata.ino(),
        })
    }
}
