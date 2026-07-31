use super::FormatFailure;

pub type FormatResult<T> = std::result::Result<T, FormatFailure>;
