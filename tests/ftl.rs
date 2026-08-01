//! FTL resourceの完全性。
//!
//! 正本localeをmessage IDの正本とし、全localeのID集合とplaceholder集合を完全一致させる。
//! 検査対象のlocaleは`locales/`にあるresourceから決めるため、言語を増やしても
//! 本fileを編集しない。規約は`locales/README.md`が持つ。

mod outcome;

use outcome::{Checked, Required, Unmet};

use std::collections::{BTreeMap, BTreeSet};

use fluent_syntax::ast;
use fluent_syntax::parser;

/// 正本locale。`src/i18n.rs`の`Locale::SOURCE`と一致させる。
const SOURCE: &str = "en";

fn locales_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("locales")
}

/// 同梱するresourceのtag。
fn locales() -> Checked<Vec<String>> {
    let mut tags: Vec<String> = Vec::new();
    for entry in
        std::fs::read_dir(locales_dir()).required_because("the locales directory is readable")?
    {
        let path = entry.required_because("directory entry")?.path();
        if path.extension().is_some_and(|extension| extension == "ftl")
            && let Some(tag) = path.file_stem().and_then(|stem| stem.to_str())
        {
            tags.push(tag.to_string());
        }
    }
    tags.sort();
    assert!(
        tags.iter().any(|tag| tag == SOURCE),
        "the source locale {SOURCE}.ftl must ship"
    );
    Ok(tags)
}

/// 正本locale以外。
fn translations() -> Checked<Vec<String>> {
    Ok(locales()?.into_iter().filter(|tag| tag != SOURCE).collect())
}

fn source(locale: &str) -> Checked<String> {
    let path = locales_dir().join(format!("{locale}.ftl"));
    std::fs::read_to_string(&path)
        .required_because(&format!("{} could not be read", path.display()))
}

/// message IDが持つ値を、resourceの原文のまま取り出す。
fn value_of(locale: &str, id: &str) -> Checked<String> {
    let text = source(locale)?;
    let prefix = format!("{id} = ");
    Ok(text
        .lines()
        .find_map(|line| line.strip_prefix(&prefix))
        .required_because(&format!("{locale}.ftl has no {id}"))?
        .to_string())
}

fn parse(locale: &str) -> Checked<ast::Resource<String>> {
    let text = source(locale)?;
    parser::parse(text)
        .map_err(|(_, errors)| Unmet::new(format!("{locale}.ftl failed to parse: {errors:?}")))
}

/// message IDと、そのmessageが参照するplaceholderの集合。
fn placeholders(locale: &str) -> Checked<BTreeMap<String, BTreeSet<String>>> {
    let resource = parse(locale)?;
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
    Ok(out)
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
fn every_built_in_locale_parses() -> Checked {
    for locale in locales()? {
        parse(&locale)?;
    }
    Ok(())
}

#[test]
fn every_locale_defines_exactly_the_same_message_ids() -> Checked {
    let expected: BTreeSet<String> = placeholders(SOURCE)?.keys().cloned().collect();

    for locale in translations()? {
        let observed: BTreeSet<String> = placeholders(&locale)?.keys().cloned().collect();

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
    Ok(())
}

#[test]
fn every_message_uses_the_same_placeholders_in_every_locale() -> Checked {
    let expected = placeholders(SOURCE)?;

    let mut mismatches = Vec::new();
    for locale in translations()? {
        let observed = placeholders(&locale)?;
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
    Ok(())
}

/// 診断が事実の行として示す値と、それを示すmessage ID。
///
/// 同じ値を説明文にも置くと、読み手は同じものを二度読むことになる。
const SHOWN_AS_A_FACT: &[(&str, &str)] = &[
    ("error-external-output-unparseable", "program"),
    ("error-external-output-unparseable", "detail"),
    ("error-external-command-failed", "program"),
    ("error-config-unreadable", "path"),
    ("error-config-unreadable", "detail"),
    ("error-config-invalid-syntax", "path"),
    ("error-config-invalid-syntax", "detail"),
    ("error-config-invalid-value", "path"),
    ("error-config-invalid-value", "field"),
    ("error-config-invalid-value", "detail"),
    ("error-git-identity-invalid", "detail"),
    ("error-metadata-invalid-value", "detail"),
    ("error-global-state-unusable", "path"),
    ("error-global-state-unusable", "detail"),
    ("error-working-directory-unusable", "path"),
    ("error-working-directory-unusable", "detail"),
    ("error-file-declaration-invalid-source", "index"),
    ("error-file-declaration-invalid-source", "source"),
    ("error-file-declaration-invalid-source", "detail"),
    ("error-file-declaration-invalid-destination", "index"),
    ("error-file-declaration-invalid-destination", "destination"),
    ("error-file-declaration-invalid-destination", "detail"),
    ("error-registry-unreadable", "path"),
    ("error-registry-unreadable", "detail"),
    ("error-registry-invalid-syntax", "path"),
    ("error-registry-invalid-syntax", "detail"),
    ("error-registry-invalid-value", "field"),
    ("error-registry-invalid-value", "detail"),
    ("error-metadata-unreadable", "path"),
    ("error-metadata-unreadable", "detail"),
    ("error-metadata-invalid-syntax", "path"),
    ("error-metadata-invalid-syntax", "detail"),
    ("error-metadata-invalid-value", "path"),
    ("error-metadata-invalid-value", "field"),
    ("error-invalid-branch-name", "value"),
    ("error-invalid-branch-name", "detail"),
    ("error-host-clone-unusable", "path"),
    ("error-host-clone-unusable", "detail"),
    ("error-image-collision", "image"),
    ("error-image-collision", "detail"),
    ("error-image-unusable", "image"),
    ("error-image-unusable", "detail"),
    ("warning-build-context-left-behind", "path"),
    ("warning-build-context-left-behind", "detail"),
    ("error-archive-unusable", "path"),
    ("error-archive-unusable", "detail"),
    ("error-template-unusable", "template"),
    ("error-template-unusable", "detail"),
    ("error-sandbox-unusable", "sandbox"),
    ("error-sandbox-unusable", "detail"),
    ("error-declared-file-unusable", "source"),
    ("error-declared-file-unusable", "detail"),
    ("error-sandbox-repository-unusable", "path"),
    ("error-sandbox-repository-unusable", "detail"),
    ("error-prompt-unreadable", "detail"),
    ("warning-lock-file-left-behind", "path"),
    ("warning-lock-file-left-behind", "detail"),
    ("error-project-path-unreadable", "path"),
    ("error-project-path-unreadable", "detail"),
    ("error-document-render-failed", "document"),
    ("error-document-render-failed", "detail"),
    ("error-atomic-write-failed", "path"),
    ("error-atomic-write-failed", "detail"),
    ("error-cleanup-failed", "path"),
    ("error-cleanup-failed", "detail"),
    ("error-lock-unavailable", "path"),
    ("error-lock-unavailable", "detail"),
    ("error-external-command-spawn-failed", "program"),
    ("error-external-command-spawn-failed", "detail"),
    ("error-platform-unobservable", "detail"),
    ("error-docker-unreachable", "detail"),
];

#[test]
fn a_value_shown_as_a_fact_is_not_repeated_in_the_sentence() -> Checked {
    for locale in locales()? {
        let placeholders = placeholders(&locale)?;
        for (id, variable) in SHOWN_AS_A_FACT {
            let observed = placeholders
                .get(*id)
                .required_because(&format!("{locale}.ftl defines {id}"))?;
            assert!(
                !observed.contains(*variable),
                "{locale}.ftl: {id} names ${variable} again, but the diagnostic shows it as a fact"
            );
        }
    }
    Ok(())
}

#[test]
fn security_messages_provide_a_description_and_a_remediation() -> Checked {
    for locale in locales()? {
        let ids: BTreeSet<String> = placeholders(&locale)?.keys().cloned().collect();
        let mut families: BTreeSet<String> = BTreeSet::new();
        for id in &ids {
            for suffix in ["-description", "-remediation"] {
                if let Some(family) = id.strip_prefix("security-").and_then(|rest| {
                    rest.strip_suffix(suffix)
                        .map(std::string::ToString::to_string)
                }) {
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
    Ok(())
}

#[test]
fn resources_carry_content_only() -> Checked {
    // 規約は`locales/README.md`が1箇所で持つ。resourceへコメントや見出しを書くと、
    // 言語の数だけ同じ規約を維持することになる。
    for locale in locales()? {
        for (index, line) in source(&locale)?.lines().enumerate() {
            assert!(
                !line.starts_with('#'),
                "{locale}.ftl:{} is a comment; conventions belong in locales/README.md: {line}",
                index + 1
            );
        }
    }
    Ok(())
}

#[test]
fn message_ids_are_kebab_case() -> Checked {
    for locale in locales()? {
        for id in placeholders(&locale)?.keys() {
            let name = id.split('.').next().unwrap_or(id);
            assert!(
                name.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'),
                "{locale}.ftl: {id} is not kebab-case"
            );
        }
    }
    Ok(())
}

#[test]
fn the_legend_describes_the_value_instead_of_repeating_it() -> Checked {
    // 状態値は翻訳しない。凡例は`ready: <説明>`の形で説明だけを訳す。
    for locale in translations()? {
        for value in ["ready", "missing", "error", "running", "stopped"] {
            let legend = value_of(&locale, &format!("legend-{value}"))?;
            assert!(
                legend != value,
                "{locale}.ftl: the legend for {value} must describe the value, not repeat it"
            );
        }
    }
    Ok(())
}

#[test]
fn a_sandbox_state_and_a_host_service_state_are_described_separately() -> Checked {
    // 同じ`running`でも、Sandboxの状態とhost serviceの状態は別の説明を持つ。
    for locale in locales()? {
        for value in ["running", "stopped"] {
            let service = value_of(&locale, &format!("legend-{value}"))?;
            let sandbox = value_of(&locale, &format!("legend-sandbox-{value}"))?;
            assert_ne!(
                service, sandbox,
                "{locale}.ftl: {value} means something different for a sandbox than for a service"
            );
        }
    }
    Ok(())
}

/// 実装が参照するmessage `IDだと分かるprefix`。
///
/// error IDの安定した表記と紛れないよう、prefixは表示文字列側だけを指すものにする。
const MESSAGE_PREFIXES: [&str; 12] = [
    "error-",
    "diagnostic-",
    "cause-",
    "remediation-",
    "security-",
    "legend-",
    "status-item-",
    "status-column-",
    "status-project-",
    "status-worktrees-",
    "column-",
    "select-",
];

#[test]
fn every_message_id_the_implementation_asks_for_exists() -> Checked {
    let defined: BTreeSet<String> = placeholders(SOURCE)?.keys().cloned().collect();
    let mut missing: BTreeSet<String> = BTreeSet::new();

    for path in rust_sources(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"))? {
        let text = std::fs::read_to_string(&path).required_because("the source is readable")?;
        for literal in string_literals(&text) {
            let looks_like_id = MESSAGE_PREFIXES
                .iter()
                .any(|prefix| literal.starts_with(prefix))
                && literal
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
            if looks_like_id && !defined.contains(&literal) {
                missing.insert(format!("{}: {literal}", path.display()));
            }
        }
    }

    assert!(
        missing.is_empty(),
        "the implementation asks for message IDs that {SOURCE}.ftl does not define: {missing:?}"
    );
    Ok(())
}

fn rust_sources(root: &std::path::Path) -> Checked<Vec<std::path::PathBuf>> {
    let mut found = Vec::new();
    let entries = std::fs::read_dir(root).required_because("the source directory is readable")?;
    for entry in entries {
        let path = entry.required_because("directory entry")?.path();
        if path.is_dir() {
            found.extend(rust_sources(&path)?);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            found.push(path);
        }
    }
    found.sort();
    Ok(found)
}

/// sourceに書かれた文字列literalを、escapeを飛ばして取り出す。
fn string_literals(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut characters = text.chars().peekable();
    while let Some(character) = characters.next() {
        if character != '"' {
            continue;
        }
        let mut literal = String::new();
        while let Some(inner) = characters.next() {
            match inner {
                '\\' => {
                    characters.next();
                    literal.push('?');
                }
                '"' => break,
                other => literal.push(other),
            }
        }
        found.push(literal);
    }
    found
}

#[test]
fn translated_diagnostic_labels_keep_the_source_term() -> Checked {
    // 利用者が正本localeの用語で検索できるよう「訳語 (正本localeの語)」の形式とする。
    const LABELS: [&str; 6] = [
        "status-item-config",
        "status-item-state-directory",
        "status-item-network-policy",
        "status-item-daemon",
        "status-column-item",
        "status-column-status",
    ];

    for locale in translations()? {
        for id in LABELS {
            let term = value_of(SOURCE, id)?;
            let translated = value_of(&locale, id)?;
            assert!(
                translated.contains(&format!("({term})")),
                "{locale}.ftl: {id} must keep the source term in parentheses: {translated}"
            );
        }
    }
    Ok(())
}
