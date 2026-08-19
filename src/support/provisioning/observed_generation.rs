/// ある世代のimageが既にあるかを1回観測した結果。
///
/// どの世代を観測したのかを一緒に持つ。別の世代の判断へ流用されないようにし、
/// 同じimageを二度観測しないために持ち回る。
pub(crate) struct ObservedGeneration {
    pub(super) dockerfile_sha256: String,
    pub(super) built: bool,
}
