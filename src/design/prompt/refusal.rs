/// 直前の確定を受け付けなかった理由。一覧の直上へ警告として出す。
///
/// 説明なく同じpromptを描き直さないために持つ。理由は1つだけ示し、新しい理由が前の理由
/// を置き換える。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Refusal {
    /// 複数選択で、1件も選ばずに確定しようとした。
    Empty,
    /// 一致する候補が無い絞り込みの上で確定しようとした。
    NoMatch,
}
