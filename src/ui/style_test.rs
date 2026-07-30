use super::*;

/// 実装が写像を持つrole。variantを足したらここへも足す。
const ROLES: [Role; 11] = [
    Role::Heading,
    Role::TableHeader,
    Role::ProgressMarker,
    Role::SuccessMarker,
    Role::WarningMarker,
    Role::ErrorMarker,
    Role::Command,
    Role::Important,
    Role::Muted,
    Role::PromptCurrent,
    Role::PromptChecked,
];

const STATES: [VisualState; 4] = [
    VisualState::Positive,
    VisualState::Attention,
    VisualState::Negative,
    VisualState::Neutral,
];

#[test]
fn every_semantic_state_maps_to_the_color_the_design_assigns_it() {
    assert_eq!(
        state_style(VisualState::Positive).foreground,
        Some(Color::Green)
    );
    assert_eq!(
        state_style(VisualState::Attention).foreground,
        Some(Color::Yellow)
    );
    assert_eq!(
        state_style(VisualState::Negative).foreground,
        Some(Color::Red)
    );
    assert_eq!(state_style(VisualState::Neutral).foreground, None);
}

#[test]
fn a_neutral_state_carries_no_decoration_at_all() {
    assert!(state_style(VisualState::Neutral).is_plain());
}

#[test]
fn the_severity_markers_do_not_share_a_color() {
    let progress = role_style(Role::ProgressMarker).foreground;
    let success = role_style(Role::SuccessMarker).foreground;
    let warning = role_style(Role::WarningMarker).foreground;
    let error = role_style(Role::ErrorMarker).foreground;

    let assigned = [progress, success, warning, error];
    for (index, left) in assigned.iter().enumerate() {
        for right in assigned.iter().skip(index + 1) {
            assert_ne!(left, right, "one meaning must not share another's color");
        }
    }
}

#[test]
fn headings_use_weight_instead_of_color() {
    // 見出しは色を消しても階層が残らなければならない。
    for role in [Role::Heading, Role::TableHeader, Role::Important] {
        let style = role_style(role);
        assert!(style.bold, "{role:?} must carry weight");
    }
    assert_eq!(role_style(Role::Heading).foreground, None);
}

#[test]
fn a_command_line_is_the_only_body_text_that_is_both_bold_and_colored() {
    let command = role_style(Role::Command);
    assert!(command.bold);
    assert_eq!(command.foreground, Some(Color::Cyan));
}

#[test]
fn dim_is_reserved_for_supporting_information() {
    for role in ROLES {
        let style = role_style(role);
        assert_eq!(
            style.dim,
            matches!(role, Role::Muted | Role::TableHeader),
            "{role:?} must not fade information the reader needs first"
        );
    }
}

#[test]
fn prompt_focus_and_prompt_selection_stay_distinguishable() {
    // currentとcheckedは別の状態であり、同じ見え方にしてはならない。
    assert_ne!(
        role_style(Role::PromptCurrent),
        role_style(Role::PromptChecked)
    );
}

#[test]
fn every_role_and_state_has_a_mapping_that_does_not_panic() {
    for role in ROLES {
        let _ = role_style(role);
    }
    for state in STATES {
        let _ = state_style(state);
    }
}

#[test]
fn the_glyph_sets_agree_on_which_meanings_exist() {
    let unicode = glyphs(CharacterSet::Unicode);
    let ascii = glyphs(CharacterSet::Ascii);
    assert_eq!(unicode.all().len(), ascii.all().len());
    for glyph in unicode.all().into_iter().chain(ascii.all()) {
        assert!(!glyph.is_empty(), "a meaning must have a visible glyph");
    }
}

#[test]
fn no_built_in_glyph_can_be_drawn_as_a_multi_color_pictograph() {
    for set in [CharacterSet::Unicode, CharacterSet::Ascii] {
        for glyph in glyphs(set).all() {
            for character in glyph.chars() {
                let point = character as u32;
                assert_ne!(point, 0xFE0F, "{glyph:?} carries an emoji presentation");
                assert_ne!(
                    point, 0xFE0E,
                    "{glyph:?} carries a text presentation selector"
                );
                assert_ne!(point, 0x200D, "{glyph:?} is a ZWJ sequence");
                assert_ne!(point, 0x20E3, "{glyph:?} is a keycap sequence");
                assert!(
                    !(0x1F1E6..=0x1F1FF).contains(&point),
                    "{glyph:?} is a regional indicator"
                );
                assert!(
                    !(0x1F300..=0x1FAFF).contains(&point),
                    "{glyph:?} sits in a pictograph block"
                );
                assert!(
                    !(0x1F000..=0x1F0FF).contains(&point),
                    "{glyph:?} sits in a pictograph block"
                );
            }
        }
    }
}

#[test]
fn every_glyph_is_a_single_code_point() {
    // 合成によって環境ごとに色数と表示幅が変わる表現を持ち込まない。
    for set in [CharacterSet::Unicode, CharacterSet::Ascii] {
        for glyph in glyphs(set).all() {
            assert_eq!(
                glyph.chars().count(),
                1,
                "{glyph:?} must be one code point, not a sequence"
            );
        }
    }
}

#[test]
fn the_ascii_fallback_stays_inside_ascii() {
    for glyph in glyphs(CharacterSet::Ascii).all() {
        assert!(glyph.is_ascii(), "{glyph:?} is not an ASCII fallback");
    }
}
