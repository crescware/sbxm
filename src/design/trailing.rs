/// blockのあとに空行が要るか。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Trailing {
    /// 次のblockが来るときに空行を置く。
    Normal,
    /// 直ちに空行で閉じる。command行の直後は必ず空行になる。
    Blank,
}
