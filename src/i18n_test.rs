/// 共有した定義が、参照している側で実際に展開されること。
///
/// FTLのparseだけでは、message参照が解決されるかは分からない。
#[test]
fn a_shared_definition_is_expanded_where_it_is_referenced() {
    for locale in Locale::ALL {
        let catalog = Catalog::new(locale);
        let scopes = catalog
            .text("github-token-scopes")
            .expect("the shared definition renders");
        assert!(
            scopes.contains("Contents"),
            "{locale:?}: the permissions are named: {scopes}"
        );

        // `add`の案内と、未登録で止まったときの案内は同じ定義を出す。
        let guidance = catalog
            .text("add-next-token")
            .expect("the guidance renders");
        assert!(
            guidance.contains("Contents"),
            "{locale:?}: add repeats the permissions: {guidance}"
        );

        // 実行するcommandは翻訳resourceではなくmodelが持つため、共有定義だけを確かめる。
        let remediation = catalog
            .text("remediation-github-secret-missing")
            .expect("the remediation renders");
        assert!(remediation.contains("Contents"), "{locale:?}: {remediation}");
        assert!(
            !remediation.contains("sbx "),
            "{locale:?}: a command must not be embedded in the prose: {remediation}"
        );
    }
}

use super::*;
use crate::msg;

#[test]
fn built_in_locales_parse_and_load() {
    for locale in Locale::ALL {
        let catalog = Catalog::new(locale);
        assert_eq!(catalog.locale(), locale);
    }
}

#[test]
fn every_locale_is_defined_exactly_once_and_round_trips_through_its_tag() {
    assert_eq!(
        Locale::ALL.len(),
        DEFINITIONS.len(),
        "ALL must expose every definition"
    );
    for locale in Locale::ALL {
        let tag = locale.as_str();
        assert_eq!(
            Locale::parse_exact(tag),
            Some(locale),
            "{tag} must round-trip through the definition table"
        );
        assert_eq!(
            DEFINITIONS
                .iter()
                .filter(|definition| definition.locale == locale)
                .count(),
            1,
            "{tag} must have exactly one definition"
        );
        assert_eq!(
            DEFINITIONS
                .iter()
                .filter(|definition| definition.tag == tag)
                .count(),
            1,
            "{tag} must be claimed by exactly one locale"
        );
    }
}

#[test]
fn every_locale_tag_is_a_language_identifier() {
    for locale in Locale::ALL {
        assert_eq!(locale.langid().to_string(), locale.as_str());
    }
}

#[test]
fn exactly_one_locale_is_the_source() {
    let sources: Vec<Locale> = Locale::ALL
        .into_iter()
        .filter(|locale| locale.is_source())
        .collect();
    assert_eq!(sources, vec![Locale::SOURCE]);
}

/// `tests/ftl.rs`の`SOURCE`と一致させる。
#[test]
fn the_source_locale_tag_is_the_one_tests_ftl_checks_against() {
    assert_eq!(Locale::SOURCE.as_str(), "en");
}

#[test]
fn language_tags_are_normalized_to_the_primary_subtag() {
    assert_eq!(Locale::from_language_tag("ja"), Some(Locale::Ja));
    assert_eq!(Locale::from_language_tag("ja-JP"), Some(Locale::Ja));
    assert_eq!(Locale::from_language_tag("ja_JP.UTF-8"), Some(Locale::Ja));
    assert_eq!(Locale::from_language_tag("en_US.UTF-8"), Some(Locale::En));
    assert_eq!(Locale::from_language_tag("C"), Some(Locale::En));
    // 組み込みlocaleにならないtagを使う。どの言語を足してもこのtestは意味を保つ。
    assert_eq!(Locale::from_language_tag("zz-ZZ"), None);
    assert_eq!(Locale::from_language_tag(""), None);
}

#[test]
fn exact_language_values_reject_anything_else() {
    assert_eq!(Locale::parse_exact("en"), Some(Locale::En));
    assert_eq!(Locale::parse_exact("ja"), Some(Locale::Ja));
    assert_eq!(Locale::parse_exact("ja-JP"), None);
    assert_eq!(Locale::parse_exact("EN"), None);
    assert_eq!(Locale::parse_exact(""), None);
}

#[test]
fn unknown_message_ids_fail_with_the_id_and_locale() {
    let catalog = Catalog::new(Locale::En);
    let failure = catalog
        .format(&msg!("no-such-message-id"))
        .expect_err("unknown message IDs must not format");
    assert_eq!(failure.message_id, "no-such-message-id");
    assert_eq!(failure.locale, Locale::En);
    assert_eq!(failure.reason, FormatFailureReason::UnknownMessage);
    assert!(failure.to_string().contains("message-format-failed"));
}

#[test]
fn formatting_does_not_insert_isolation_marks() {
    let catalog = Catalog::new(Locale::En);
    let rendered = catalog
        .format(&msg!(
            "error-config-missing",
            path = "/home/example/.sbxm/config.yaml"
        ))
        .expect("built-in message must format");
    assert!(
        rendered.contains("/home/example/.sbxm/config.yaml"),
        "argument must be interpolated verbatim: {rendered}"
    );
    assert!(
        !rendered.contains('\u{2068}') && !rendered.contains('\u{2069}'),
        "output must be free of Unicode isolation marks: {rendered:?}"
    );
}
