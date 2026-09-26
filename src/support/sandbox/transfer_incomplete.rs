/// Sandboxの中で受け取ったbyte列を置く手順が、期待するdigestと一致しない内容を
/// 受け取ったことを示す終了status。
///
/// 宣言fileは、hostから送ったbyte列を確かめてから置く。
pub const TRANSFER_INCOMPLETE: i32 = 65;
