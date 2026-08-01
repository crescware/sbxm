use std::path::Path;

use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::i18n::Locale;
use crate::msg;
use crate::paths::{self};

use super::{
    ConfigState, DOCUMENT_VERSION, GlobalConfig, RawConfig, invalid_value, missing_field,
    parse_files, parse_git_identity, supported_language_list, unknown_key_warnings,
};

/// configのtextを検証する。filesystemには触れない部分の判定をまとめる。
pub(super) fn parse(text: &str, path: &Path) -> Result<ConfigState> {
    let syntax_error = |error: yaml_serde::Error| {
        Error::single(
            Diagnostic::new(
                ErrorId::ConfigInvalidSyntax,
                msg!("error-config-invalid-syntax"),
            )
            .fact(Fact::path(&paths::display(path)))
            .fact(Fact::cause(&error.to_string())),
        )
    };

    let document: yaml_serde::Value = yaml_serde::from_str(text).map_err(syntax_error)?;
    // 空のdocumentもcommentだけのdocumentもnullとして読める。keyを1つも持たない
    // mappingと同じ扱いにし、欠落したfieldをsyntax errorではなく名前で報告する。
    let document = if document.is_null() {
        yaml_serde::Value::Mapping(yaml_serde::Mapping::new())
    } else {
        document
    };

    let warnings = unknown_key_warnings(&document, path);

    let raw: RawConfig = yaml_serde::from_value(document).map_err(syntax_error)?;

    DOCUMENT_VERSION.require(raw.version, &paths::display(path), || {
        missing_field(path, "version")
    })?;

    // 欠落した`language`は未保存であり、不正な値とは別の状態である。
    let language = match raw.language {
        Some(value) => Some(Locale::parse_exact(&value).ok_or_else(|| {
            invalid_value(
                path,
                "language",
                &format!("{value} is not one of {}", supported_language_list()),
            )
        })?),
        None => None,
    };

    let git_identity = parse_git_identity(raw.git_user_name, raw.git_user_email, path)?;

    let files = parse_files(raw.files, path)?;

    Ok(ConfigState::Valid {
        config: Box::new(GlobalConfig {
            language,
            git_identity,
            files,
        }),
        warnings,
    })
}
