use std::process::{Child, ExitStatus};
use std::time::Duration;

use crate::design::ExternalOutput;
use crate::diagnostics::Result;

use super::{CommandSpec, pump_until_exit};

/// 外部toolが出したbyteを、届いた順に`ExternalOutput`へ渡す。
///
/// 端末をそのまま貸すのではなく1度sbxmを通すことで、sbxmの行との境界も、見せない行の
/// 判断も描画側へ寄せられる。byteは溜めずに届いたまま流すため、復帰文字で書き換わる
/// 進捗表示もそのまま動く。
///
/// 端末を共有するcommandはprocess groupを分けない。利用者のCtrl-Cは子processへ直接
/// 届くため、割り込みを見張る相手をこの実行は持たない。
pub(super) fn run_relay(
    child: &mut Child,
    spec: &CommandSpec,
    limit: Option<Duration>,
    output: &mut dyn ExternalOutput,
) -> Result<ExitStatus> {
    pump_until_exit(child, spec, limit, None, &mut |_, bytes| {
        output.relay(bytes);
    })
}
