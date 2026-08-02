use super::*;

/// 状態値と、翻訳しない綴り、凡例のFTL message ID。
const DECLARED: [(StatusValue, &str, &str); 6] = [
    (StatusValue::Defaults, "defaults", "legend-defaults"),
    (StatusValue::Ready, "ready", "legend-ready"),
    (StatusValue::Missing, "missing", "legend-missing"),
    (StatusValue::Error, "error", "legend-error"),
    (StatusValue::Running, "running", "legend-running"),
    (StatusValue::Stopped, "stopped", "legend-stopped"),
];

#[test]
fn every_status_value_carries_its_own_untranslated_spelling_and_legend() {
    for (value, spelling, legend) in DECLARED {
        assert_eq!(value.as_str(), spelling);
        assert_eq!(value.legend_id(), legend);
    }

    // 綴りが重なると、表を読む側は2つの状態を区別できない。凡例も同じ理由で別々に持つ。
    let mut spellings: Vec<&str> = DECLARED.iter().map(|(_, spelling, _)| *spelling).collect();
    spellings.sort_unstable();
    spellings.dedup();
    assert_eq!(spellings.len(), DECLARED.len());
    let mut legends: Vec<&str> = DECLARED.iter().map(|(_, _, legend)| *legend).collect();
    legends.sort_unstable();
    legends.dedup();
    assert_eq!(legends.len(), DECLARED.len());
}

#[test]
fn a_status_value_displays_the_spelling_it_carries_rather_than_its_variant_name() {
    for (value, spelling, _) in DECLARED {
        // 表示側が`as_str`を呼び忘れても綴りが変わらないよう、Displayは同じ綴りを出す。
        assert_eq!(format!("{value}"), spelling);
        assert_eq!(format!("{value}"), value.as_str());
        assert_ne!(format!("{value}"), format!("{value:?}"));
    }
}
