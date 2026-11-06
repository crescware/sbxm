/// `git init --bare`から、originとfetch refspecの設定までの工程の進み具合。
///
/// 中断の跡と言えるのは、sbxmが途中まで作ったと確かめられた場合だけである。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Initialization {
    /// 初期化markerが残った、またはまだ空のdirectory。利用者の成果物は無い。
    Uninitialized,
    /// 中断の跡ではない。設定し終えたか、sbxmが途中まで作ったとは言えない。
    Settled,
    /// originもfetch refspecも無く、利用者のrefもobjectも無い。
    OriginMissing,
    /// 宣言したoriginだけが1つあり、fetch refspecが無い。
    FetchRefspecMissing,
}
