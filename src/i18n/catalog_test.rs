use fluent_bundle::{FluentBundle, FluentResource};

use super::{Catalog, Locale};

impl Catalog {
    /// `id`のmessageだけを持たない辞書。
    ///
    /// 組み込みの辞書は使う鍵をすべて持つ。鍵が欠けたときに止まる経路は、この辞書で踏む。
    pub(crate) fn without(locale: Locale, id: &str) -> Catalog {
        let heading = format!("{id} =");
        let mut kept = String::new();
        let mut inside = false;
        for line in locale.source().lines() {
            // 続きの行は字下げで始まる。見出しの次の字下げの無い行で、そのmessageは終わる。
            inside = line.starts_with(&heading)
                || (inside && (line.starts_with(' ') || line.starts_with('\t')));
            if !inside {
                kept.push_str(line);
                kept.push('\n');
            }
        }
        Catalog::from_source(locale, kept)
    }

    /// `source`のFTLだけを持つ辞書。
    fn from_source(locale: Locale, source: String) -> Catalog {
        let resource =
            FluentResource::try_new(source).unwrap_or_else(|(resource, _errors)| resource);
        let mut bundle = FluentBundle::new(vec![locale.langid()]);
        bundle.set_use_isolating(false);
        let _ = bundle.add_resource(resource);
        Catalog { locale, bundle }
    }
}

#[test]
fn a_message_that_has_only_attributes_is_reported_as_missing_its_value() {
    let catalog = Catalog::from_source(
        Locale::En,
        "attributes-only =\n    .label = shown elsewhere\n".to_string(),
    );

    let failure = catalog.text("attributes-only");

    assert!(
        matches!(
            failure.as_ref().map_err(|failure| &failure.reason),
            Err(super::FormatFailureReason::MissingValue)
        ),
        "{failure:?}"
    );
}
