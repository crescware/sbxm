use std::path::PathBuf;

use super::InputBytes;

/// 子のstdinへ渡すもの。
///
/// byte列とfileは同時には渡せない。どちらを渡すかを1つの値で持ち、片方が黙って
/// 無視される組み合わせを作らない。
#[derive(Debug, Clone)]
pub(super) enum CommandInput {
    /// 何も渡さない。stdinを待つ子にも、すぐEOFを届ける。
    Empty,
    /// sbxmが持つbyte列を書き込み、書き終えたら閉じる。
    Bytes(InputBytes),
    /// fileをつなぐ。sbxmは中身を読まず、子が読み切ればEOFになる。
    File(PathBuf),
}
