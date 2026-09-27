use super::Processes;

/// 時間切れの子processを終わらせ、終了statusを回収する。
///
/// 打ち切る時点で相手は期限内に応答しなかった。猶予を与えても終わる保証は増えないため、
/// 直ちに終了signalを送る（契約test C10）。
pub(super) fn terminate_child<O: Processes>(os: &O, child: &mut O::Child) {
    let _ = os.end_child(child);
    // 終了statusを引き取り、zombieを残さない。
    let _ = os.wait_exit(child);
}
