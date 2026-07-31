use std::fs::File;

/// 保持している間だけ保護区間を占有するOS file lock。
///
/// lock fileはworkflow終了後も削除しない。fileの存在自体は処理中を意味せず、
/// lock取得の成否だけが排他の根拠となる。
#[derive(Debug)]
pub struct ExclusiveLock {
    pub(super) file: File,
}

impl Drop for ExclusiveLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}
