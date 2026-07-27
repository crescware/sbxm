//! FTL resourceの完全性。
//!
//! 正本localeをmessage IDの正本とし、全localeのID集合とplaceholder集合を完全一致させる。
//! 検査対象のlocaleは`locales/`にあるresourceから決めるため、言語を増やしても
//! 本fileを編集しない。規約は`locales/README.md`が持つ。

use std::collections::{BTreeMap, BTreeSet};

use fluent_syntax::ast;
use fluent_syntax::parser;

/// 正本locale。`src/i18n.rs`の`Locale::SOURCE`と一致させる。
const SOURCE: &str = "en";

fn locales_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("locales")
}

/// 同梱するresourceのtag。
fn locales() -> Vec<String> {
    let mut tags: Vec<String> = std::fs::read_dir(locales_dir())
        .expect("the locales directory is readable")
        .map(|entry| entry.expect("directory entry").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "ftl"))
        .filter_map(|path| {
            path.file_stem()
                .and_then(|stem| stem.to_str())
                .map(|stem| stem.to_string())
        })
        .collect();
    tags.sort();
    assert!(
        tags.iter().any(|tag| tag == SOURCE),
        "the source locale {SOURCE}.ftl must ship"
    );
    tags
}

/// 正本locale以外。
fn translations() -> Vec<String> {
    locales().into_iter().filter(|tag| tag != SOURCE).collect()
}

fn source(locale: &str) -> String {
    let path = locales_dir().join(format!("{locale}.ftl"));
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{} could not be read: {error}", path.display()))
}

/// message IDが持つ値を、resourceの原文のまま取り出す。
fn value_of(locale: &str, id: &str) -> String {
    let text = source(locale);
    let prefix = format!("{id} = ");
    text.lines()
        .find_map(|line| line.strip_prefix(&prefix))
        .unwrap_or_else(|| panic!("{locale}.ftl has no {id}"))
        .to_string()
}

fn parse(locale: &str) -> ast::Resource<String> {
    let text = source(locale);
    match parser::parse(text) {
        Ok(resource) => resource,
        Err((_, errors)) => panic!("{locale}.ftl failed to parse: {errors:?}"),
    }
}

/// message IDと、そのmessageが参照するplaceholderの集合。
fn placeholders(locale: &str) -> BTreeMap<String, BTreeSet<String>> {
    let resource = parse(locale);
    let mut out: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

    for entry in &resource.body {
        let ast::Entry::Message(message) = entry else {
            continue;
        };
        let id = message.id.name.clone();
        let mut variables = BTreeSet::new();
        if let Some(pattern) = &message.value {
            collect_pattern(pattern, &mut variables);
        }
        assert!(
            out.insert(id.clone(), variables).is_none(),
            "{locale}.ftl defines {id} more than once"
        );

        for attribute in &message.attributes {
            let attribute_id = format!("{}.{}", message.id.name, attribute.id.name);
            let mut variables = BTreeSet::new();
            collect_pattern(&attribute.value, &mut variables);
            assert!(
                out.insert(attribute_id.clone(), variables).is_none(),
                "{locale}.ftl defines {attribute_id} more than once"
            );
        }
    }
    out
}

fn collect_pattern(pattern: &ast::Pattern<String>, out: &mut BTreeSet<String>) {
    for element in &pattern.elements {
        if let ast::PatternElement::Placeable { expression } = element {
            collect_expression(expression, out);
        }
    }
}

fn collect_expression(expression: &ast::Expression<String>, out: &mut BTreeSet<String>) {
    match expression {
        ast::Expression::Inline(inline) => collect_inline(inline, out),
        ast::Expression::Select { selector, variants } => {
            collect_inline(selector, out);
            for variant in variants {
                collect_pattern(&variant.value, out);
            }
        }
    }
}

fn collect_inline(inline: &ast::InlineExpression<String>, out: &mut BTreeSet<String>) {
    match inline {
        ast::InlineExpression::VariableReference { id } => {
            out.insert(id.name.clone());
        }
        ast::InlineExpression::Placeable { expression } => collect_expression(expression, out),
        ast::InlineExpression::FunctionReference { arguments, .. } => {
            for positional in &arguments.positional {
                collect_inline(positional, out);
            }
            for named in &arguments.named {
                collect_inline(&named.value, out);
            }
        }
        _ => {}
    }
}

#[test]
fn every_built_in_locale_parses() {
    for locale in locales() {
        parse(&locale);
    }
}

#[test]
fn every_locale_defines_exactly_the_same_message_ids() {
    let expected: BTreeSet<String> = placeholders(SOURCE).keys().cloned().collect();

    for locale in translations() {
        let observed: BTreeSet<String> = placeholders(&locale).keys().cloned().collect();

        let missing: Vec<&String> = expected.difference(&observed).collect();
        let extra: Vec<&String> = observed.difference(&expected).collect();

        assert!(
            missing.is_empty(),
            "{locale}.ftl is missing message IDs defined in the source of truth: {missing:?}"
        );
        assert!(
            extra.is_empty(),
            "{locale}.ftl defines message IDs that {SOURCE}.ftl does not: {extra:?}"
        );
    }
}

#[test]
fn every_message_uses_the_same_placeholders_in_every_locale() {
    let expected = placeholders(SOURCE);

    let mut mismatches = Vec::new();
    for locale in translations() {
        let observed = placeholders(&locale);
        for (id, expected) in &expected {
            let Some(observed) = observed.get(id) else {
                continue;
            };
            if expected != observed {
                mismatches.push(format!("{id}: {SOURCE}={expected:?} {locale}={observed:?}"));
            }
        }
    }
    assert!(
        mismatches.is_empty(),
        "placeholder sets differ from the source locale:\n{}",
        mismatches.join("\n")
    );
}

#[test]
fn security_messages_provide_a_description_and_a_remediation() {
    for locale in locales() {
        let ids: BTreeSet<String> = placeholders(&locale).keys().cloned().collect();
        let mut families: BTreeSet<String> = BTreeSet::new();
        for id in &ids {
            for suffix in ["-description", "-remediation"] {
                if let Some(family) = id
                    .strip_prefix("security-")
                    .and_then(|rest| rest.strip_suffix(suffix).map(|family| family.to_string()))
                {
                    families.insert(family);
                }
            }
        }
        assert!(
            !families.is_empty(),
            "{locale}.ftl defines no security messages"
        );
        for family in families {
            for suffix in ["description", "remediation"] {
                let id = format!("security-{family}-{suffix}");
                assert!(
                    ids.contains(&id),
                    "{locale}.ftl is missing {id}; security messages need both"
                );
            }
        }
    }
}

#[test]
fn resources_carry_content_only() {
    // 規約は`locales/README.md`が1箇所で持つ。resourceへコメントや見出しを書くと、
    // 言語の数だけ同じ規約を維持することになる。
    for locale in locales() {
        for (index, line) in source(&locale).lines().enumerate() {
            assert!(
                !line.starts_with('#'),
                "{locale}.ftl:{} is a comment; conventions belong in locales/README.md: {line}",
                index + 1
            );
        }
    }
}

#[test]
fn message_ids_are_kebab_case() {
    for locale in locales() {
        for id in placeholders(&locale).keys() {
            let name = id.split('.').next().unwrap_or(id);
            assert!(
                name.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'),
                "{locale}.ftl: {id} is not kebab-case"
            );
        }
    }
}

#[test]
fn the_legend_describes_the_value_instead_of_repeating_it() {
    // 状態値は翻訳しない。凡例は`ready: <説明>`の形で説明だけを訳す。
    for locale in translations() {
        for value in ["ready", "missing", "error", "running", "stopped"] {
            let legend = value_of(&locale, &format!("legend-{value}"));
            assert!(
                legend != value,
                "{locale}.ftl: the legend for {value} must describe the value, not repeat it"
            );
        }
    }
}

#[test]
fn translated_diagnostic_labels_keep_the_source_term() {
    // 利用者が正本localeの用語で検索できるよう「訳語 (正本localeの語)」の形式とする。
    const LABELS: [&str; 6] = [
        "status-item-config",
        "status-item-base-path",
        "status-item-network-policy",
        "status-item-daemon",
        "status-column-item",
        "status-column-status",
    ];

    for locale in translations() {
        for id in LABELS {
            let term = value_of(SOURCE, id);
            let translated = value_of(&locale, id);
            assert!(
                translated.contains(&format!("({term})")),
                "{locale}.ftl: {id} must keep the source term in parentheses: {translated}"
            );
        }
    }
}
