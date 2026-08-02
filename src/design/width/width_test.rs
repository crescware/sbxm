use crate::testing::outcome::{Checked, Required};

use super::*;

/// 2桁として数える符号位置の範囲。実装が並べる表と同じ順で持つ。
const WIDE_RANGES: [(u32, u32); 12] = [
    (0x1100, 0x115F),
    (0x2E80, 0x303E),
    (0x3041, 0x33FF),
    (0x3400, 0x4DBF),
    (0x4E00, 0x9FFF),
    (0xA000, 0xA4CF),
    (0xAC00, 0xD7A3),
    (0xF900, 0xFAFF),
    (0xFE30, 0xFE6F),
    (0xFF00, 0xFF60),
    (0xFFE0, 0xFFE6),
    (0x20000, 0x3FFFD),
];

/// どの範囲にも属さない符号位置か。隣り合う範囲の境目を外側と数えないために要る。
fn outside_every_range(code_point: u32) -> bool {
    !WIDE_RANGES
        .iter()
        .any(|(start, end)| (*start..=*end).contains(&code_point))
}

#[test]
fn every_range_counts_two_columns_from_its_first_code_point_to_its_last() -> Checked {
    for (start, end) in WIDE_RANGES {
        for inside in [start, end] {
            let character =
                char::from_u32(inside).required_because("the boundary is a code point")?;
            assert_eq!(
                display_width(&character.to_string()),
                2,
                "U+{inside:04X} is inside a full-width range"
            );
        }
    }
    Ok(())
}

#[test]
fn a_code_point_next_to_a_range_but_outside_every_range_counts_one_column() -> Checked {
    // 表の端を1つ外すだけで幅が変わることを、範囲ごとに固定する。
    for (start, end) in WIDE_RANGES {
        for outside in [start - 1, end + 1] {
            // 範囲どうしが隣接する境目は、外側ではなく次の範囲の内側である。
            if !outside_every_range(outside) {
                continue;
            }
            let character =
                char::from_u32(outside).required_because("the neighbour is a code point")?;
            assert_eq!(
                display_width(&character.to_string()),
                1,
                "U+{outside:04X} lies outside every full-width range"
            );
        }
    }
    Ok(())
}

#[test]
fn wide_characters_count_as_two_columns() {
    assert_eq!(display_width("Config"), 6);
    assert_eq!(display_width("設定 (Config)"), 4 + 9);
    assert_eq!(display_width(""), 0);
}

#[test]
fn padding_reaches_the_requested_column() {
    assert_eq!(padding("ab", 5), "   ");
    assert_eq!(padding("設定", 6), "  ");
}

#[test]
fn padding_never_pulls_a_column_backwards() {
    // 想定より広いcellがあっても、負の余白で列を壊さない。
    assert_eq!(padding("a much longer value", 4), "");
}

#[test]
fn a_label_that_fits_is_left_alone() {
    assert_eq!(truncate("owner/repository", 16), "owner/repository");
}

#[test]
fn a_label_that_does_not_fit_keeps_its_beginning() {
    assert_eq!(truncate("owner/repository", 10), "owner/r...");
    assert_eq!(display_width(&truncate("owner/repository", 10)), 10);
}

#[test]
fn truncation_counts_full_width_characters_as_two_columns() {
    let truncated = truncate("案件名がとても長い", 8);
    assert!(display_width(&truncated) <= 8, "{truncated}");
    assert!(truncated.ends_with("..."), "{truncated}");
}

#[test]
fn a_width_too_small_for_the_ellipsis_still_stays_inside_it() {
    assert_eq!(display_width(&truncate("owner/repository", 2)), 2);
}
