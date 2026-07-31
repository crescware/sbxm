/// remote-tracking refの接頭辞。
const ORIGIN_REF_PREFIX: &str = "refs/remotes/origin/";

/// `refs/remotes/origin/<branch>`
pub fn origin_ref(branch: &str) -> String {
    format!("{ORIGIN_REF_PREFIX}{branch}")
}
