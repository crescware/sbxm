use super::{HTTPS_CLONE_URL_FORM, SSH_CLONE_URL_FORM};

/// 受理する形式の一覧。error本文へ観測値と並べて示す。
pub fn accepted_clone_url_forms() -> String {
    format!("{SSH_CLONE_URL_FORM}, {HTTPS_CLONE_URL_FORM}")
}
