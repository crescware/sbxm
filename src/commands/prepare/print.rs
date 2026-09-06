//! `prepare`の出力。

use crate::design::{Document, GuidanceItem};
use crate::i18n::Locale;
use crate::msg;

use crate::commands::present;
use crate::support::provisioning::ProvisioningOutput;

/// `prepare`が並べるもの。
///
/// 構築の結果そのものは`open`と共有する。この入口だけの次の一手を末尾へ足す。
pub fn print(output: &ProvisioningOutput, locale: Locale) -> Document {
    present::provisioning_output(output, locale)
        // 案件IDを打ち直させない。次のcommandはそのままcopyできる形で出す。
        .guidance(
            Some(msg!("add-next-heading")),
            vec![GuidanceItem::Plain(msg!("add-next-open"))],
        )
        .try_command(format!("sbxm open {}", output.project))
}
