use super::*;

#[test]
fn wide_characters_count_as_two_columns() {
    assert_eq!(display_width("Config"), 6);
    assert_eq!(display_width("設定 (Config)"), 4 + 9);
    assert_eq!(display_width(""), 0);
}
