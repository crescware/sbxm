/// `--version`が表示する文字列。
pub fn version_line() -> String {
    format!("sbxm {}", env!("CARGO_PKG_VERSION"))
}
