//! 表示言語とFTL resource。
//!
//! すべての利用者向け文字列をFTL resourceから生成する。正本localeのFTLをmessage IDの
//! 正本とし、enum、path、command、exit status、外部stdout/stderrは翻訳しない。
//!
//! 言語ごとの内容は`locales/<tag>.ftl`だけが持ち、言語ごとの同一性は本moduleの
//! [`DEFINITIONS`]だけが持つ。ほかの場所へ言語別の分岐を置かない。

use std::str::FromStr;

use fluent_bundle::{FluentArgs, FluentBundle, FluentResource, FluentValue};
use unic_langid::LanguageIdentifier;

use crate::error::Msg;

/// 組み込みlocale。variantが言語のidentityであり、内容はFTLと[`DEFINITIONS`]が持つ。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Locale {
    En,
    Ja,
}

/// 1 localeの定義。
struct LocaleDefinition {
    locale: Locale,
    /// `--lang`とconfigで使う安定した表記。翻訳しない。
    tag: &'static str,
    /// 組み込みFTL resource。
    ftl: &'static str,
}

/// 組み込みlocaleの定義。
///
/// 言語を増やすときは、[`Locale`]のvariantとこの表の行、そして`locales/<tag>.ftl`だけを
/// 足す。ほかのmoduleとtestは、この表からの導出だけを見る。
const DEFINITIONS: [LocaleDefinition; 2] = [EN, JA];

const EN: LocaleDefinition = LocaleDefinition {
    locale: Locale::En,
    tag: "en",
    ftl: include_str!("../locales/en.ftl"),
};

const JA: LocaleDefinition = LocaleDefinition {
    locale: Locale::Ja,
    tag: "ja",
    ftl: include_str!("../locales/ja.ftl"),
};

impl Locale {
    /// message IDの正本であり、fallbackであり、翻訳しない状態値が書かれている言語。
    pub const SOURCE: Locale = Locale::En;

    /// 組み込みlocaleの全体。[`DEFINITIONS`]の並び順をそのまま公開する。
    pub const ALL: [Locale; DEFINITIONS.len()] = {
        let mut all = [Locale::SOURCE; DEFINITIONS.len()];
        let mut index = 0;
        while index < DEFINITIONS.len() {
            all[index] = DEFINITIONS[index].locale;
            index += 1;
        }
        all
    };

    /// 正本localeか。
    ///
    /// 状態値を翻訳しない契約により、正本locale以外は状態値へ注釈を必要とする。
    pub fn is_source(self) -> bool {
        self == Locale::SOURCE
    }

    /// `--lang`とconfigで使う安定した表記。翻訳しない。
    pub fn as_str(self) -> &'static str {
        self.definition().tag
    }

    /// `--lang`とconfigの`language`が受け付ける厳密な値。
    pub fn parse_exact(value: &str) -> Option<Locale> {
        DEFINITIONS
            .iter()
            .find(|definition| definition.tag == value)
            .map(|definition| definition.locale)
    }

    /// macOS優先言語やshell localeのようなtagからの推測。
    ///
    /// 組み込みlocaleのtagと一致した場合だけ確定させ、その他は寄せない（呼び出し側で
    /// fallbackを決める）。
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
        // localeを持たないshell環境は正本localeとして扱う。
        if primary == "c" || primary == "posix" {
            return Some(Locale::SOURCE);
        }
        Locale::parse_exact(&primary)
    }

    /// variantから定義への対応。variantを足すと、この写像が網羅を強制する。
    fn definition(self) -> &'static LocaleDefinition {
        match self {
            Locale::En => &EN,
            Locale::Ja => &JA,
        }
    }

    /// `FTLのbundleへ渡すlanguage` identifier。tagから導出する。
    ///
    /// tagは表が持つASCIIのlanguage subtagであり、読めない値は入らない。読めない場合でも
    /// bundleは未定言語として組み上がり、message解決そのものは変わらない。tagとidentifierの
    /// 一致はtestが固定する。
    fn langid(self) -> LanguageIdentifier {
        LanguageIdentifier::from_str(self.as_str()).unwrap_or_default()
    }

    fn source(self) -> &'static str {
        self.definition().ftl
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

/// `FTLのformatに失敗したという内部異常`。
///
/// 利用者向け文字列を生成できない状態であるため、対象message `IDとlocaleを英語で示す`。
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
    /// `組み込みFTLからcatalogを作る`。
    ///
    /// 組み込みresourceのparse失敗はbuild成果物の不備であり、testで検出する。
    ///
    /// 実行時は読めた範囲で組み上げる。欠けたmessageはmessage IDとして表に出るため、
    /// 不備は隠れずに現れる。
    pub fn new(locale: Locale) -> Self {
        let resource = FluentResource::try_new(locale.source().to_owned())
            .unwrap_or_else(|(resource, _errors)| resource);
        let mut bundle = FluentBundle::new(vec![locale.langid()]);
        // 出力を機械的に比較できるようにするため、方向性制御文字を挿入しない。
        bundle.set_use_isolating(false);
        // 重複IDの報告は無視する。localeごとのID集合はtestが固定する。
        let _ = bundle.add_resource(resource);
        Catalog { locale, bundle }
    }

    pub fn locale(&self) -> Locale {
        self.locale
    }

    /// message `IDと引数からlocalizedな文字列を作る`。
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
#[path = "i18n_test.rs"]
mod i18n_test;
