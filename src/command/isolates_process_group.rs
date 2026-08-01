use super::{CommandSpec, OutputPolicy};

/// 専用のprocess groupで起動するcommandか。
///
/// 出力をpipeで受け取るcommandは、時間で打ち切るときに、そのcommandが起動したprocessまで
/// 終わらせないと書き込み端が閉じない。専用のgroupへ置けば、group宛のsignal 1つで
/// 子孫まで含めて終わりが決まる。捕捉した出力を読むのはsbxmだけなので、利用者の端末は
/// この分離の影響を受けない。
///
/// 端末へ出す2つのpolicyは、sbxmと同じgroupに残す。別のgroupへ移すと利用者のCtrl-Cが
/// 子processへ届かず、sbxmが終わったあとも端末へ書き続ける相手が残る。そのため、これらの
/// timeoutで終わらせるのは直接の子だけとする。
pub(super) fn isolates_process_group(spec: &CommandSpec) -> bool {
    spec.output == OutputPolicy::Capture
}
