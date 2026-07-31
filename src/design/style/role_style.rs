use super::{Color, Role, StyleSpec};

/// roleに対応する装飾。
pub fn role_style(role: Role) -> StyleSpec {
    match role {
        // 見出しと照合の基準は、どちらも周囲から1段だけ前へ出す。
        Role::Heading | Role::Important => StyleSpec::bold(),
        // 列名は読み飛ばす対象であり、階層は示すが本文より前へ出さない。
        Role::TableHeader => StyleSpec {
            bold: true,
            dim: true,
            ..StyleSpec::plain()
        },
        Role::ProgressMarker => StyleSpec::color(Color::Cyan),
        // 成功と選択済みは、どちらも「満たされている」ことを同じ緑で示す。
        Role::SuccessMarker | Role::PromptChecked => StyleSpec::color(Color::Green),
        Role::WarningMarker => StyleSpec::color(Color::Yellow),
        Role::ErrorMarker => StyleSpec::bold_color(Color::Red),
        // 入力する一行とfocusのある行は、どちらも「いま手を動かす対象」である。
        Role::Command | Role::PromptCurrent => StyleSpec::bold_color(Color::Cyan),
        Role::Muted => StyleSpec {
            dim: true,
            ..StyleSpec::plain()
        },
    }
}
