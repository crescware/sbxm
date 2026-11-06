use crate::diagnostics::Error;

/// 時間切れまでに届いたstderrを、captureを終える前に診断へ移す。
pub(super) fn with_captured_stderr(mut error: Error, stderr: Vec<u8>) -> Error {
    if let Error::Diagnostics(diagnostics) = &mut error
        && let Some(external) = diagnostics
            .first_mut()
            .and_then(|item| item.external.as_mut())
    {
        external.stderr_lossy = std::str::from_utf8(&stderr).is_err();
        external.stderr = stderr;
    }
    error
}
