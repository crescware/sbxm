use std::str::FromStr;
use std::sync::OnceLock;

use unic_langid::LanguageIdentifier;

use super::{DEFINITIONS, EN, JA, LocaleDefinition};

/// 組み込みlocale。variantが言語のidentityであり、内容はFTLと[`DEFINITIONS`]が持つ。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Locale {
    En,
    Ja,
}

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

    /// `--lang`とconfigが受け付ける安定した表記の全体。
    pub fn accepted_values() -> impl ExactSizeIterator<Item = &'static str> {
        Self::ALL.into_iter().map(Self::as_str)
    }

    /// `--lang`のvalue name。parser libraryが`&'static str`を要求するため一度だけ組む。
    pub fn value_name() -> &'static str {
        static VALUE_NAME: OnceLock<String> = OnceLock::new();
        VALUE_NAME
            .get_or_init(|| Self::accepted_values().collect::<Vec<_>>().join("|"))
            .as_str()
    }

    /// helpとdiagnosticへ並べる、受け付けるlocale tagの一覧。
    pub fn value_list() -> String {
        Self::accepted_values().collect::<Vec<_>>().join(", ")
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
    pub(crate) fn langid(self) -> LanguageIdentifier {
        LanguageIdentifier::from_str(self.as_str()).unwrap_or_default()
    }

    pub(crate) fn source(self) -> &'static str {
        self.definition().ftl
    }
}

impl std::fmt::Display for Locale {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
