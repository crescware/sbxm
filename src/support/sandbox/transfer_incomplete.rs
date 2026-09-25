/// Sandboxの中で受け取ったbyte列を置く手順が、期待するdigestと一致しない内容を
/// 受け取ったことを示す終了status。
///
/// 宣言fileもbundleも、hostから送ったbyte列を同じ規則で確かめてから置く。
pub const TRANSFER_INCOMPLETE: i32 = 65;
