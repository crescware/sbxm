use super::*;

#[test]
fn the_tests_run_on_the_git_the_host_is_asked_for() {
    // `mise run test-min-git`は、`scripts/min-git/build.sh`がこのversionでbuildするgitで
    // testを走らせる。hostに求めるversionと食い違えば、そのversionで動くことを確かめて
    // いないことになる。
    let built = include_str!("../../../scripts/min-git/version");
    assert_eq!(built.trim_end(), MINIMUM_HOST_GIT.to_string());
}
