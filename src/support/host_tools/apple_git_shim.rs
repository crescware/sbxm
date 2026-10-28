/// macOSが`/usr/bin`に置くgit。
///
/// Xcode Command Line Toolsのgitへ振り分けるshimであり、file自身は変わらないまま、
/// 振り分け先のversionだけが変わる。識別情報が同じでもversionが同じとは言えない。
pub(super) const APPLE_GIT_SHIM: &str = "/usr/bin/git";
