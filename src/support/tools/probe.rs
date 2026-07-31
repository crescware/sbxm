use super::ALL;

/// Sandbox内のtoolを一度に数えるscript。
///
/// 1つも無い場合も成功で終え、標準出力へ並ぶ名前で答える。exit statusで分けると、
/// 「toolが無い」と「検査自体が実行できなかった」を区別できない。
pub fn probe() -> String {
    let names: Vec<&str> = ALL.iter().map(|tool| tool.name()).collect();
    format!(
        "for c in {}; do command -v \"$c\" > /dev/null 2>&1 && printf '%s\\n' \"$c\"; done",
        names.join(" ")
    )
}
