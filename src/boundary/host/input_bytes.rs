/// 子のstdinへ渡すbyte列。
///
/// 宣言fileの中身のように、表示や記録へ出してはならないbyteを運ぶ。debug表示には長さだけを
/// 出し、中身を出さない。
#[derive(Clone, PartialEq, Eq)]
pub struct InputBytes(Vec<u8>);

impl InputBytes {
    pub fn new(bytes: Vec<u8>) -> InputBytes {
        InputBytes(bytes)
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.0
    }
}

impl std::fmt::Debug for InputBytes {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "InputBytes({} bytes)", self.0.len())
    }
}
