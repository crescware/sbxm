/// `fetch`がtagを追従するかどうか。翻訳しない安定したenum。
///
/// 既定の`Auto`は、取得したobjectを指すtagをopportunisticに追従し、ローカルの
/// `refs/tags/*`へ書き込む。この書き込みは観測ではなく状態変更であり、破壊操作の
/// 可否を観測するfetchでこれが起きると、同じrepositoryへの2回目の観測が1回目とは
/// 違うローカルref集合を読むことになる。観測目的のfetchは`Disabled`で呼ぶ。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagFollowing {
    Auto,
    Disabled,
}
