use super::InputBytes;

/// 子のstdinへ渡すもの。
#[derive(Debug, Clone)]
pub(super) enum CommandInput {
    /// 何も渡さない。stdinを待つ子にも、すぐEOFを届ける。
    Empty,
    /// sbxmが持つbyte列を書き込み、書き終えたら閉じる。
    Bytes(InputBytes),
}
