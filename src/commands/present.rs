//! domain値を表示語彙へ写す。
//!
//! domain enumはUIの色を持たない。同じ`stopped`でも、停止commandの完了結果ならpositive、
//! 稼働要件のstatusならattentionである。文脈を知っているのはcommandであり、enumではない。
//! そのため`value -> color`のglobalな表を作らず、この層で`VisualState`を明示する。
//!
//! 凡例も同じ理由でここに置く。状態値は翻訳しないため、正本locale以外の正常出力には
//! 出現した値の説明を添える。表へ値を置くたびに凡例へ控えられるよう、写像と登録を
//! 1回の呼び出しにまとめる。

use std::collections::BTreeMap;

use crate::compatibility::SandboxState;
use crate::error::Msg;
use crate::i18n::Locale;
use crate::metadata::CreationMode;
use crate::msg;
use crate::support::files::Placement;
use crate::support::inventory::{Observed, ProjectState};
use crate::support::status::StatusValue;
use crate::ui::{Inline, LegendEntry, VisualState};

use super::status::project::Value as ProjectValue;
use super::stop::run::StopResult;

/// global環境の要件を満たしているか。
pub fn global_status(value: StatusValue) -> Inline {
    let state = match value {
        StatusValue::Ready | StatusValue::Running => VisualState::Positive,
        // 宣言が無いことは、既定で動くという答えそのものである。
        StatusValue::Defaults => VisualState::Neutral,
        // 要件として見た場合、不在も停止も行動を促す。
        StatusValue::Missing | StatusValue::Stopped => VisualState::Attention,
        StatusValue::Error => VisualState::Negative,
    };
    Inline::state(value.as_str(), state)
}

/// 1案件の項目の観測結果。
pub fn project_status(value: ProjectValue) -> Inline {
    let state = match value {
        ProjectValue::Ready
        | ProjectValue::Running
        | ProjectValue::Clean
        | ProjectValue::NotExposed => VisualState::Positive,
        ProjectValue::Missing
        | ProjectValue::Mismatch
        | ProjectValue::Changed
        | ProjectValue::Stopped
        | ProjectValue::NotCreated
        | ProjectValue::Dirty
        | ProjectValue::NotObservedStopped => VisualState::Attention,
        // hostの鍵で署名できる状態は、注意ではなく失敗として示す。
        ProjectValue::Exposed => VisualState::Negative,
        // 構成の別と、見に行く対象がないことは、良し悪しではない。
        ProjectValue::Attached | ProjectValue::Detached | ProjectValue::NotApplicable => {
            VisualState::Neutral
        }
    };
    Inline::state(value.as_str(), state)
}

/// 一覧に並ぶSandboxの状態。
pub fn project_state(state: ProjectState) -> Inline {
    let visual = match state {
        ProjectState::Running => VisualState::Positive,
        ProjectState::Stopped | ProjectState::NotCreated => VisualState::Attention,
    };
    Inline::state(state.as_str(), visual)
}

/// registry entryから観測した案件の状態。
///
/// registryと成果物が食い違っている状態は、注意ではなく失敗として示す。
pub fn observed(observed: &Observed) -> Inline {
    match observed {
        Observed::Registered(state) => project_state(*state),
        Observed::Incomplete => Inline::state(observed.as_str(), VisualState::Attention),
        Observed::Missing | Observed::Inconsistent => {
            Inline::state(observed.as_str(), VisualState::Negative)
        }
    }
}

/// 構築後のSandboxの状態。
pub fn sandbox_state(state: SandboxState) -> Inline {
    let visual = match state {
        SandboxState::Running => VisualState::Positive,
        SandboxState::Stopped => VisualState::Attention,
    };
    Inline::state(state.as_str(), visual)
}

/// 停止commandの結果。停止できたことは成功である。
pub fn stop_result(result: StopResult) -> Inline {
    let visual = match result {
        StopResult::Stopped => VisualState::Positive,
        StopResult::Unchanged => VisualState::Neutral,
        StopResult::Failed => VisualState::Negative,
    };
    Inline::state(result.as_str(), visual)
}

/// worktreeの作成mode。良し悪しではなく構成の別である。
pub fn creation_mode(mode: CreationMode) -> Inline {
    Inline::state(mode.to_string(), VisualState::Neutral)
}

/// 宣言fileの配置結果。
pub fn placement(placement: Placement) -> Inline {
    let visual = match placement {
        Placement::Placed => VisualState::Positive,
        Placement::Unchanged => VisualState::Neutral,
    };
    Inline::state(placement.as_str(), visual)
}

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

    pub fn observed(&mut self, value: &Observed) -> Inline {
        self.cell(observed(value), value.legend_id())
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

#[cfg(test)]
#[path = "present_test.rs"]
mod present_test;
