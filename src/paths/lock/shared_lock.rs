use std::fs::File;

/// 保持している間だけ保護区間を共有占有するOS file lock。
///
/// 複数の保持者が同時に取得できる。exclusive lockとは同じfileに対して排他する。
///
/// lock fileはworkflow終了後も削除しない。fileの存在自体は処理中を意味せず、
/// lock取得の成否だけが排他の根拠となる。
#[derive(Debug)]
pub struct SharedLock {
    pub(super) file: File,
}

impl Drop for SharedLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}
