use crate::design::{StreamPolicy, Trace};
use crate::i18n::Locale;
use crate::time::Clock;

/// processのstderrを、`--verbose`のtraceの書き先にする。
pub(crate) fn create_trace(locale: Locale, policy: StreamPolicy, clock: &dyn Clock) -> Trace<'_> {
    Trace::new(locale, policy, std::io::stderr(), clock)
}
