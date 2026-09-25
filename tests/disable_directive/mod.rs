//! architecture testの検査を、FTL resourceの次の1行だけ無効にする宣言。
//!
//! 書き方と、理由を必ず添えることは`locales/README.md`が持つ。宣言を読んで検査を
//! 無効にするのは`tests/architecture.rs`であり、`tests/ftl.rs`はresourceへ書いてよい
//! commentをこの宣言だけに限る。
//!
//! 2本のtest binaryが同じ実体を取り込むため、両方が使うものだけを置く。

/// 宣言の行の書き出し。
pub const DISABLE_NEXT_LINE: &str = "# architecture-test-disable-next-line";
