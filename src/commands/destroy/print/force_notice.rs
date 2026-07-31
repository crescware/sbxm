use crate::design::Warning;
use crate::msg;

/// force modeが省く検査。結果ではなく注意である。
pub fn force_notice() -> Warning {
    Warning::text(msg!("destroy-force-notice"))
}
