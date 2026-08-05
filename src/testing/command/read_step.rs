/// `ScriptedPipe`が1回の読みで返すもの。
pub enum ReadStep {
    Bytes(&'static [u8]),
    Interrupted,
    WouldBlock,
    Failed,
}
