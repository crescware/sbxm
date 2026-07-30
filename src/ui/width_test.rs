use super::*;

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
