/// `df -Pk /`から読んだ、root filesystemの使用量。
///
/// KiB単位のinteger。filesystemの`Size`は空き容量の計算に使わないため保持しない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RootDiskUsage {
    /// `Available`列。予約領域を含まない、実際に書き込める量。
    pub free_kib: u64,
    /// `Used + Available`。予約領域を除いた実効天井。
    pub usable_kib: u64,
    /// `Capacity`列（`df`の第5列）をそのまま保持する。
    pub capacity_percent: u8,
}
