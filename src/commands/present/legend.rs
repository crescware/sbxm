use std::collections::BTreeMap;

use crate::compatibility::SandboxState;
use crate::design::{Inline, LegendEntry};
use crate::diagnostics::Msg;
use crate::i18n::Locale;
use crate::metadata::CreationMode;
use crate::msg;
use crate::support::files::Placement;
use crate::support::status::StatusValue;

use crate::commands::status::project::Value as ProjectValue;
use crate::commands::stop::StopResult;

use super::{
    ListState, creation_mode, global_status, placement, project_status, sandbox_state, stop_result,
};

/// Sandboxの状態を説明するmessage ID。host serviceの説明を流用しない。
fn sandbox_state_legend(state: SandboxState) -> &'static str {
    match state {
        SandboxState::Running => "legend-sandbox-running",
        SandboxState::Stopped => "legend-sandbox-stopped",
    }
}

fn creation_mode_legend(mode: CreationMode) -> &'static str {
    match mode {
        CreationMode::Attached => "legend-attached",
        CreationMode::Detached => "legend-detached",
    }
}

fn placement_legend(placement: Placement) -> &'static str {
    match placement {
        Placement::Placed => "legend-placed",
        Placement::Unchanged => "legend-unchanged",
    }
}

/// 出現した状態値の凡例。
///
/// 状態値を翻訳しない契約により、正本locale以外は説明を必要とする。値の重複は畳み、
/// 出現しなかった値は並べない。
pub struct Legend {
    locale: Locale,
    entries: BTreeMap<String, &'static str>,
}

impl Legend {
    pub fn new(locale: Locale) -> Legend {
        Legend {
            locale,
            entries: BTreeMap::new(),
        }
    }

    /// 表示した値と、その説明のFTL message IDを控える。
    pub fn add(&mut self, value: &str, description: &'static str) {
        self.entries.insert(value.to_string(), description);
    }

    /// cellを控えたうえでそのまま返す。表への追加と凡例への登録を1度に書ける。
    pub fn cell(&mut self, cell: Inline, description: &'static str) -> Inline {
        self.add(cell.as_str(), description);
        cell
    }

    pub fn global_status(&mut self, value: StatusValue) -> Inline {
        self.cell(global_status(value), value.legend_id())
    }

    pub fn project_status(&mut self, value: ProjectValue) -> Inline {
        self.cell(project_status(value), value.legend_id())
    }

    pub fn list_state(&mut self, value: ListState) -> Inline {
        self.cell(value.render(), value.legend_id())
    }

    pub fn sandbox_state(&mut self, state: SandboxState) -> Inline {
        self.cell(sandbox_state(state), sandbox_state_legend(state))
    }

    pub fn stop_result(&mut self, result: StopResult) -> Inline {
        self.cell(stop_result(result), result.legend_id())
    }

    pub fn creation_mode(&mut self, mode: CreationMode) -> Inline {
        self.cell(creation_mode(mode), creation_mode_legend(mode))
    }

    pub fn placement(&mut self, value: Placement) -> Inline {
        self.cell(placement(value), placement_legend(value))
    }

    /// 凡例のheading。
    pub fn heading() -> Msg {
        msg!("legend-heading")
    }

    /// 凡例の行。正本localeでは空とし、sectionごと省かせる。
    pub fn entries(self) -> Vec<LegendEntry> {
        if self.locale.is_source() {
            return Vec::new();
        }
        self.entries
            .into_iter()
            .map(|(value, description)| LegendEntry::new(value, msg!(description)))
            .collect()
    }
}
