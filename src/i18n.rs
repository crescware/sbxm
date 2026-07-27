//! 表示言語とFTL resource。
//!
//! すべての利用者向け文字列をFTL resourceから生成する。英語FTLをmessage IDの正本とし、
//! enum、path、command、exit status、外部stdout/stderrは翻訳しない。

use fluent_bundle::{FluentArgs, FluentBundle, FluentResource, FluentValue};
use unic_langid::{LanguageIdentifier, langid};

use crate::error::Msg;

/// 組み込みlocale。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Locale {
    En,
    Ja,
}

/// 英語FTLをmessage IDの正本とする。
pub const SOURCE_FTL: &str = include_str!("../locales/en.ftl");
const JA_FTL: &str = include_str!("../locales/ja.ftl");

impl Locale {
    /// 組み込みlocaleの全体。
    pub const ALL: [Locale; 2] = [Locale::En, Locale::Ja];

    /// `--lang`とconfigで使う安定した表記。翻訳しない。
    pub fn as_str(self) -> &'static str {
        match self {
            Locale::En => "en",
            Locale::Ja => "ja",
        }
    }

    /// `--lang`とconfigの`language`が受け付ける厳密な値。
    pub fn parse_exact(value: &str) -> Option<Locale> {
        match value {
            "en" => Some(Locale::En),
            "ja" => Some(Locale::Ja),
            _ => None,
        }
    }

    /// macOS優先言語やshell localeのようなtagからの推測。
    ///
    /// `ja`または`ja-*`を日本語とし、その他は英語へ寄せない（呼び出し側でfallbackを決める）。
    pub fn from_language_tag(tag: &str) -> Option<Locale> {
        let normalized = tag.trim();
        if normalized.is_empty() {
            return None;
        }
        // `ja_JP.UTF-8`、`ja-JP`、`ja`のいずれも先頭のsubtagだけを見る。
        let primary = normalized
            .split(['-', '_', '.', '@'])
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        match primary.as_str() {
            "ja" => Some(Locale::Ja),
            "en" => Some(Locale::En),
            "c" | "posix" => Some(Locale::En),
            _ => None,
        }
    }

    fn langid(self) -> LanguageIdentifier {
        match self {
            Locale::En => langid!("en"),
            Locale::Ja => langid!("ja"),
        }
    }

    fn source(self) -> &'static str {
        match self {
            Locale::En => SOURCE_FTL,
            Locale::Ja => JA_FTL,
        }
    }
}

impl std::fmt::Display for Locale {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// shell localeから表示言語を推測する。
///
/// `LC_ALL`、`LC_MESSAGES`、`LANG`の順に見る。
pub fn shell_locale() -> Option<Locale> {
    for key in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Some(value) = std::env::var_os(key)
            && let Some(locale) = Locale::from_language_tag(&value.to_string_lossy())
        {
            return Some(locale);
        }
    }
    None
}

/// FTL format失敗の理由。localeに依存せず英語で表示する最終手段。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormatFailureReason {
    UnknownMessage,
    MissingValue,
    MissingAttribute,
    Format(String),
}

/// FTLのformatに失敗したという内部異常。
///
/// 利用者向け文字列を生成できない状態であるため、対象message IDとlocaleを英語で示す。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatFailure {
    pub message_id: String,
    pub locale: Locale,
    pub reason: FormatFailureReason,
}

impl std::fmt::Display for FormatFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let reason = match &self.reason {
            FormatFailureReason::UnknownMessage => "message is not defined".to_string(),
            FormatFailureReason::MissingValue => "message has no value".to_string(),
            FormatFailureReason::MissingAttribute => "attribute is not defined".to_string(),
            FormatFailureReason::Format(detail) => detail.clone(),
        };
        write!(
            f,
            "message-format-failed: message-id={} locale={} reason={}",
            self.message_id, self.locale, reason
        )
    }
}

pub type FormatResult<T> = std::result::Result<T, FormatFailure>;

/// 1 localeぶんのFTL resource。
pub struct Catalog {
    locale: Locale,
    bundle: FluentBundle<FluentResource>,
}

impl Catalog {
    /// 組み込みFTLからcatalogを作る。
    ///
    /// 組み込みresourceのparse失敗はbuild成果物の不備であり、testで検出する。
    pub fn new(locale: Locale) -> Self {
        let resource =
            FluentResource::try_new(locale.source().to_owned()).unwrap_or_else(|(_, errors)| {
                panic!("built-in {locale} FTL failed to parse: {errors:?}")
            });
        let mut bundle = FluentBundle::new(vec![locale.langid()]);
        // 出力を機械的に比較できるようにするため、方向性制御文字を挿入しない。
        bundle.set_use_isolating(false);
        bundle
            .add_resource(resource)
            .unwrap_or_else(|errors| panic!("built-in {locale} FTL failed to load: {errors:?}"));
        Catalog { locale, bundle }
    }

    pub fn locale(&self) -> Locale {
        self.locale
    }

    /// message IDと引数からlocalizedな文字列を作る。
    pub fn format(&self, message: &Msg) -> FormatResult<String> {
        let mut args = FluentArgs::new();
        for (key, value) in &message.args {
            args.set(*key, FluentValue::from(value.as_str()));
        }
        self.format_pattern(message.id, None, Some(&args))
    }

    /// 引数のないmessageを取り出す。
    pub fn text(&self, id: &str) -> FormatResult<String> {
        self.format_pattern(id, None, None)
    }

    /// messageのattributeを取り出す。
    pub fn attribute(&self, id: &str, attribute: &str) -> FormatResult<String> {
        self.format_pattern(id, Some(attribute), None)
    }

    /// message IDが定義されているか。
    pub fn has(&self, id: &str) -> bool {
        self.bundle.get_message(id).is_some()
    }

    fn format_pattern(
        &self,
        id: &str,
        attribute: Option<&str>,
        args: Option<&FluentArgs>,
    ) -> FormatResult<String> {
        let failure = |reason: FormatFailureReason| FormatFailure {
            message_id: match attribute {
                Some(attribute) => format!("{id}.{attribute}"),
                None => id.to_string(),
            },
            locale: self.locale,
            reason,
        };

        let message = self
            .bundle
            .get_message(id)
            .ok_or_else(|| failure(FormatFailureReason::UnknownMessage))?;

        let pattern = match attribute {
            Some(attribute) => message
                .get_attribute(attribute)
                .map(|attribute| attribute.value())
                .ok_or_else(|| failure(FormatFailureReason::MissingAttribute))?,
            None => message
                .value()
                .ok_or_else(|| failure(FormatFailureReason::MissingValue))?,
        };

        let mut errors = Vec::new();
        let formatted = self.bundle.format_pattern(pattern, args, &mut errors);
        if !errors.is_empty() {
            return Err(failure(FormatFailureReason::Format(format!("{errors:?}"))));
        }
        Ok(formatted.into_owned())
    }
}

#[cfg(test)]
mod tests {
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
    fn language_tags_are_normalized_to_the_primary_subtag() {
        assert_eq!(Locale::from_language_tag("ja"), Some(Locale::Ja));
        assert_eq!(Locale::from_language_tag("ja-JP"), Some(Locale::Ja));
        assert_eq!(Locale::from_language_tag("ja_JP.UTF-8"), Some(Locale::Ja));
        assert_eq!(Locale::from_language_tag("en_US.UTF-8"), Some(Locale::En));
        assert_eq!(Locale::from_language_tag("C"), Some(Locale::En));
        assert_eq!(Locale::from_language_tag("fr-FR"), None);
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
                path = "/home/example/.sbxm/config.toml"
            ))
            .expect("built-in message must format");
        assert!(
            rendered.contains("/home/example/.sbxm/config.toml"),
            "argument must be interpolated verbatim: {rendered}"
        );
        assert!(
            !rendered.contains('\u{2068}') && !rendered.contains('\u{2069}'),
            "output must be free of Unicode isolation marks: {rendered:?}"
        );
    }
}
