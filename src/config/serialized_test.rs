use crate::testing::outcome::{Checked, Refused, Required};

use super::*;

/// serializeが必ず失敗する値。
///
/// 本番のRaw型は文字列と整数とVecだけで組み立てるため、この失敗は本番の型では起こせない。
/// 起こせないことと、起きたときに壊れた記録を書かないことは別であり、後者はserializerを
/// 失敗させて確かめる。
struct Unserializable;

impl serde::Serialize for Unserializable {
    fn serialize<S: serde::Serializer>(
        &self,
        _serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        Err(serde::ser::Error::custom(
            "this value has no representation",
        ))
    }
}

/// 診断が持つ、その項目名の事実。
fn fact_value(diagnostic: &Diagnostic, label: &str) -> Checked<String> {
    diagnostic
        .facts
        .iter()
        .find_map(|fact| match fact {
            Fact::OneLine { label: name, value } if name.id == label => {
                Some(value.as_str().to_string())
            }
            _ => None,
        })
        .required_because("the diagnostic states this fact")
}

#[test]
fn a_value_that_cannot_be_serialized_is_refused_instead_of_written_broken() -> Checked {
    let error = serialized(&Unserializable, "registry.yaml")
        .refused_because("a value that has no YAML is not written")?;

    let diagnostic = error
        .diagnostics()
        .first()
        .required_because("one diagnostic")?;
    assert_eq!(diagnostic.id, ErrorId::DocumentRenderFailed);
    assert_eq!(diagnostic.description.id, "error-document-render-failed");
    // どの記録を組み立てられなかったかは呼び出し側しか知らない。errorがその名前を預かる。
    assert_eq!(
        fact_value(diagnostic, "diagnostic-document-label")?,
        "registry.yaml"
    );
    // 原因はserializerが書いた原文であり、sbxmが言い換えない。
    assert!(
        fact_value(diagnostic, "diagnostic-cause-label")?
            .contains("this value has no representation"),
        "the cause the serializer reported is kept: {:?}",
        diagnostic.facts
    );
    Ok(())
}

#[test]
fn the_document_name_travels_with_the_failure_it_belongs_to() -> Checked {
    // 同じ失敗でも、どのfileを書こうとしていたかで読み手のすることが変わる。
    let error =
        serialized(&Unserializable, "config.yaml").refused_because("serialization fails")?;
    let diagnostic = error
        .diagnostics()
        .first()
        .required_because("one diagnostic")?;
    assert_eq!(
        fact_value(diagnostic, "diagnostic-document-label")?,
        "config.yaml"
    );
    Ok(())
}

#[test]
fn a_value_that_serializes_becomes_the_document_text() -> Checked {
    let mut mapping = yaml_serde::Mapping::new();
    mapping.insert(
        yaml_serde::Value::String("version".to_string()),
        yaml_serde::Value::Number(1.into()),
    );
    assert_eq!(
        serialized(&mapping, "config.yaml").required_because("a mapping serializes")?,
        "version: 1\n"
    );
    Ok(())
}
