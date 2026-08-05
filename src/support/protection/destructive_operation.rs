/// rebuildとdestroyで、保護ゲートの判定が変わる点。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DestructiveOperation {
    /// 管理外の作業ツリーは層Aで拒否する。rebuildは同じ配置を再作成できない。
    Rebuild,
    /// 管理外の作業ツリーは内容を層Aで検査したうえで、存在自体を層Bへ載せる。
    Destroy,
}
