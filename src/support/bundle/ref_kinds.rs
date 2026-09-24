/// bundleが運ぶrefの種類と、hostの名前空間の中でそれを置く名前。
///
/// Sandbox側の`refs/sbxm/save/`は、各worktreeのHEADを置く一時refである。
pub(super) const REF_KINDS: [(&str, &str); 3] = [
    ("refs/heads/", "heads/"),
    ("refs/tags/", "tags/"),
    ("refs/sbxm/save/", "worktrees/"),
];
