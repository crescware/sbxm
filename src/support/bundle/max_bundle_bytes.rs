/// Sandboxから受け取るbundle 1件の上限。
///
/// bundleは履歴全体を運ぶため、宣言fileよりはるかに大きくなりうる。上限はhostの
/// diskを使い切らないための歯止めであり、repositoryの大きさの目安ではない。
pub const MAX_BUNDLE_BYTES: u64 = 16 * 1024 * 1024 * 1024;
