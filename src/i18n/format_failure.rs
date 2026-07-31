use super::{FormatFailureReason, Locale};

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
