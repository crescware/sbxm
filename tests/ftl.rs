//! FTL resourceの完全性。
//!
//! 英語FTLをmessage IDの正本とし、英語と日本語のID集合およびplaceholder集合を
//! 完全一致させる。組み込みlocaleの欠落とplaceholder不一致はtest failureとする。

use std::collections::{BTreeMap, BTreeSet};

use fluent_syntax::ast;
use fluent_syntax::parser;

const LOCALES: [&str; 2] = ["en", "ja"];

fn source(locale: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("locales")
        .join(format!("{locale}.ftl"));
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{} could not be read: {error}", path.display()))
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
    for locale in LOCALES {
        parse(locale);
    }
}

#[test]
fn the_locales_define_exactly_the_same_message_ids() {
    let english: BTreeSet<String> = placeholders("en").keys().cloned().collect();
    let japanese: BTreeSet<String> = placeholders("ja").keys().cloned().collect();

    let missing: Vec<&String> = english.difference(&japanese).collect();
    let extra: Vec<&String> = japanese.difference(&english).collect();

    assert!(
        missing.is_empty(),
        "ja.ftl is missing message IDs defined in the source of truth: {missing:?}"
    );
    assert!(
        extra.is_empty(),
        "ja.ftl defines message IDs that en.ftl does not: {extra:?}"
    );
}

#[test]
fn every_message_uses_the_same_placeholders_in_both_locales() {
    let english = placeholders("en");
    let japanese = placeholders("ja");

    let mut mismatches = Vec::new();
    for (id, expected) in &english {
        let Some(observed) = japanese.get(id) else {
            continue;
        };
        if expected != observed {
            mismatches.push(format!("{id}: en={expected:?} ja={observed:?}"));
        }
    }
    assert!(
        mismatches.is_empty(),
        "placeholder sets differ between locales:\n{}",
        mismatches.join("\n")
    );
}

#[test]
fn security_messages_provide_a_title_a_description_and_a_remediation() {
    for locale in LOCALES {
        let ids: BTreeSet<String> = placeholders(locale).keys().cloned().collect();
        let mut families: BTreeSet<String> = BTreeSet::new();
        for id in &ids {
            for suffix in ["-title", "-description", "-remediation"] {
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
            for suffix in ["title", "description", "remediation"] {
                let id = format!("security-{family}-{suffix}");
                assert!(
                    ids.contains(&id),
                    "{locale}.ftl is missing {id}; security messages need all three"
                );
            }
        }
    }
}

#[test]
fn message_ids_are_kebab_case() {
    for locale in LOCALES {
        for id in placeholders(locale).keys() {
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
fn enum_values_are_not_translated_in_the_japanese_resource() {
    // 状態値は翻訳しない。凡例は`ready: <説明>`の形で説明だけを訳す。
    let japanese = source("ja");
    for value in ["ready", "missing", "error", "running", "stopped"] {
        assert!(
            !japanese.contains(&format!("legend-{value} = {value}")),
            "the legend for {value} must describe the value, not repeat it"
        );
    }
}

#[test]
fn japanese_diagnostic_labels_keep_the_english_term() {
    // 日本語の診断labelは「日本語 (English)」とする。
    let japanese = source("ja");
    for (id, english) in [
        ("status-item-config", "Config"),
        ("status-item-base-path", "Base path"),
        ("status-item-network-policy", "Network policy"),
        ("status-item-session-inspection", "Session inspection"),
        ("status-column-item", "ITEM"),
        ("status-column-status", "STATUS"),
    ] {
        let line = japanese
            .lines()
            .find(|line| line.starts_with(&format!("{id} = ")))
            .unwrap_or_else(|| panic!("ja.ftl has no {id}"));
        assert!(
            line.contains(&format!("({english})")),
            "{id} must keep the English term in parentheses: {line}"
        );
    }
}
