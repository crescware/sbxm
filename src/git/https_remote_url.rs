use super::GITHUB_HOST;

/// `Sandbox内のcloneが使うHTTPS` remote。
pub fn https_remote_url(owner: &str, repository: &str) -> String {
    format!("https://{GITHUB_HOST}/{owner}/{repository}.git")
}
