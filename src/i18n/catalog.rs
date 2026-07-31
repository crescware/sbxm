use fluent_bundle::{FluentArgs, FluentBundle, FluentResource, FluentValue};

use crate::diagnostics::Msg;

use super::{FormatFailure, FormatFailureReason, FormatResult, Locale};

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
