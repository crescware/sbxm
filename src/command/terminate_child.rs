use std::process::Child;

use super::{CommandSpec, isolates_process_group};

/// 時間切れの子processを終わらせ、終了statusを回収する。
///
/// 打ち切る時点で相手は期限内に応答しなかった。猶予を与えても終わる保証は増えないため、
/// 捕まえられる範囲へ直ちに終了signalを送る。
pub(super) fn terminate_child(child: &mut Child, spec: &CommandSpec) {
    if isolates_process_group(spec) {
        // 専用groupで起動したcommandは、そこから起動されたprocessごと終わらせる。
        // groupにはこの子自身も含まれる。
        let _ = rustix::process::kill_process_group(
            rustix::process::Pid::from_child(child),
            rustix::process::Signal::KILL,
        );
    }
    // 端末を共有するcommandはgroupを分けていない。直接の子だけを終わらせる。
    // group宛のsignalが届かなかった場合も、ここで子自身の終了は確定する。
    let _ = child.kill();
    // 終了statusを引き取り、zombieを残さない。
    let _ = child.wait();
}
