/// KiBをGiB単位の値へ、小数第1位まで四捨五入して表示する。
///
/// floatへ変換せず、u128の整数演算だけで丸める。
pub fn format_gib(kib: u64) -> String {
    const KIB_PER_GIB: u128 = 1024 * 1024;
    let tenths = (u128::from(kib) * 10 + KIB_PER_GIB / 2) / KIB_PER_GIB;
    format!("{}.{} GiB", tenths / 10, tenths % 10)
}
