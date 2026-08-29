use std::process::Child;

/// 時間切れの子processを終わらせ、終了statusを回収する。
///
/// 打ち切る時点で相手は期限内に応答しなかった。猶予を与えても終わる保証は増えないため、
/// 直ちに終了signalを送る。
pub(super) fn terminate_child(child: &mut Child) {
    let _ = child.kill();
    // 終了statusを引き取り、zombieを残さない。
    let _ = child.wait();
}
