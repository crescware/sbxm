/// Git identityの値として使えるか。
pub fn validate_git_identity_value(value: &str) -> std::result::Result<(), &'static str> {
    if value.trim().is_empty() {
        return Err("the value is empty");
    }
    if value.contains('\n') || value.contains('\r') {
        return Err("the value contains a line break");
    }
    Ok(())
}
