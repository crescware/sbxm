use std::fs::{File, TryLockError};

/// file lockの基本操作。待たずに試し、取れなければ`WouldBlock`を返す。
pub struct SystemFileLock;

impl SystemFileLock {
    /// 排他lockを試す。
    pub fn try_lock(file: &File) -> Result<(), TryLockError> {
        file.try_lock()
    }

    /// 共有lockを試す。
    pub fn try_lock_shared(file: &File) -> Result<(), TryLockError> {
        file.try_lock_shared()
    }

    /// lockを外す。
    pub fn unlock(file: &File) -> std::io::Result<()> {
        file.unlock()
    }
}

#[cfg(test)]
#[path = "system_file_lock_test.rs"]
mod system_file_lock_test;
