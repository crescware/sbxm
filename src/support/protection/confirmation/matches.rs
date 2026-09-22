use super::super::ProtectionSnapshot;

/// `entered`が対象を名指しているか。
///
/// 名指しと認めるのは、案件の登録ID（`<owner>/<repository>`）と、そこから導いたsandbox名の
/// 2つである。訊くのは登録IDの側とする。sandbox名は末尾にsbxmが内部で使うhashを持ち、
/// 利用者が覚えている値ではないためである。画面が示すsandbox名を打ち返した場合も、同じ
/// 対象を名指したものとして受け取る。
///
/// 大文字小文字は、登録IDの比較の正本（ASCII lowercase形式）と同じく区別しない。空文字列は
/// どちらとも一致させない。非対話環境が「答える手段が無い」ことを示すために渡す値であり、
/// 一致の候補ではないためである。
///
/// `snapshot`を消費しないため、打ち直しを許す[`super::confirm_interactively`]が1回ごとの
/// 判定に使う。何を一致とみなすかはこの関数だけが決め、[`super::confirm`]もここを通る。
pub(super) fn matches(snapshot: &ProtectionSnapshot, entered: &str) -> bool {
    if entered.is_empty() {
        return false;
    }
    entered.eq_ignore_ascii_case(snapshot.assessment.project())
        || entered.eq_ignore_ascii_case(snapshot.assessment.sandbox().as_str())
}
