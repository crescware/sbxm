use std::process::ExitStatus;
use std::time::Duration;

use crate::design::ExternalOutput;
use crate::diagnostics::Result;
use crate::time::Clock;

use super::{CommandSpec, Pipes, pump_until_exit};

/// 外部toolが出したbyteを、届いた順に`ExternalOutput`へ渡す。
///
/// 端末をそのまま貸すのではなく1度sbxmを通すことで、sbxmの行との境界も、見せない行の
/// 判断も描画側へ寄せられる。byteは溜めずに届いたまま流すため、復帰文字で書き換わる
/// 進捗表示もそのまま動く。
///
/// この実行の子はsbxmと同じprocess groupに留まる（契約test C13）。利用者のCtrl-Cは子processへ
/// 直接届くため、割り込みを見張る相手をこの実行は持たない。
pub(super) fn run_relay<O: Pipes>(
    os: &O,
    clock: &dyn Clock,
    child: &mut O::Child,
    spec: &CommandSpec,
    limit: Option<Duration>,
    output: &mut dyn ExternalOutput,
) -> Result<ExitStatus> {
    pump_until_exit(os, clock, child, spec, limit, &|| false, &mut |_, bytes| {
        output.relay(bytes);
        Ok(())
    })
}
