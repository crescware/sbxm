use super::GITHUB_HOST;

pub(super) fn require_github(host: &str) -> Option<()> {
    host.eq_ignore_ascii_case(GITHUB_HOST).then_some(())
}
