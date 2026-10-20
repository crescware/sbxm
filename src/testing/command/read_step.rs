/// `ScriptedPipe`が1回の読みで返すもの。
///
/// byte列は、読み手の用意した長さまでを返し、残りを次の読みへ回す。
pub enum ReadStep {
    Bytes(&'static [u8]),
    /// testが組み立てたbyte列。
    Owned(Vec<u8>),
    Interrupted,
    WouldBlock,
    Failed,
}
