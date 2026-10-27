use std::rc::Rc;

use crate::design::{StreamPolicy, Trace};
use crate::i18n::Locale;
use crate::time::Clock;

/// processのstderrを、`--verbose`のtraceの書き先にする。
pub(crate) fn create_trace(locale: Locale, policy: StreamPolicy, clock: Rc<dyn Clock>) -> Trace {
    Trace::new(locale, policy, std::io::stderr(), clock)
}
