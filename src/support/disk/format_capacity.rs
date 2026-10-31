/// 容量の絶対値で単位を選ぶ。1 GiB未満はMiB、以上はGiB小数1桁。
///
/// MiBは切り捨て、100 MiBの警告境界を表示の丸めで隠さない。KiBは表示しない。
pub fn format_capacity(kib: u64) -> String {
    const KIB_PER_MIB: u64 = 1024;
    const KIB_PER_GIB: u128 = 1024 * 1024;

    if kib > 0 && kib < KIB_PER_MIB {
        return "<1 MiB".to_string();
    }
    if u128::from(kib) < KIB_PER_GIB {
        return format!("{} MiB", kib / KIB_PER_MIB);
    }
    let tenths = (u128::from(kib) * 10 + KIB_PER_GIB / 2) / KIB_PER_GIB;
    format!("{}.{} GiB", tenths / 10, tenths % 10)
}
