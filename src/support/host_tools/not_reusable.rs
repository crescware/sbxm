/// 確認結果の再利用に適さない実行ファイルである理由。
///
/// どれも、実体の識別情報が変わらないまま、実際に動くversionだけが変わりうる形である。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum NotReusable {
    /// 解決先のfile名がprogram名と違う。miseのように、1つのbinaryが呼ばれた名前で
    /// 振り分けるshim。
    NameMismatch,
    /// `#!`で始まるscript。asdfのshimや手書きのwrapperのように、別の実行ファイルを呼ぶ。
    Script,
    /// macOSの`/usr/bin/git`。Xcode Command Line Toolsのgitへ振り分けるshim。
    AppleShim,
}

impl NotReusable {
    /// 翻訳しない安定した表記。
    pub(super) fn as_str(self) -> &'static str {
        match self {
            NotReusable::NameMismatch => "name-mismatch",
            NotReusable::Script => "script",
            NotReusable::AppleShim => "apple-shim",
        }
    }
}
