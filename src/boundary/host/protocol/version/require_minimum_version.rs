use crate::diagnostics::{Error, ErrorId, Result};
use crate::msg;

use super::{CliVersion, MINIMUM_CLI_VERSION};

/// 検出したversionが最小要件を満たすかを判定する。
pub fn require_minimum_version(observed: CliVersion) -> Result<()> {
    if observed >= MINIMUM_CLI_VERSION {
        return Ok(());
    }
    Err(Error::new(
        ErrorId::SbxVersionBelowMinimum,
        msg!(
            "error-sbx-version-below-minimum",
            observed = observed,
            minimum = MINIMUM_CLI_VERSION
        ),
    ))
}
