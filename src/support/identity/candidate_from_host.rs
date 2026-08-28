use crate::boundary::host::{CommandSpec, HostEnvironment, TimeoutClass};
use crate::metadata::validate_git_identity_value;

/// 入力欄へ置くhostの候補を読む。
///
/// 候補であって決定ではないため、読めない場合を失敗として扱わない。不在、空、複数値、
/// 観測不能のいずれも空文字とし、利用者が自分で打てる空欄として現れる。
///
/// `--get-all`は複数回宣言された値をすべて返す。1つに絞れない設定から候補を選ばない。
pub fn candidate_from_host(host: &dyn HostEnvironment, key: &str) -> String {
    let spec = CommandSpec::probe("git", &["config", "--global", "--get-all", key])
        .timeout(TimeoutClass::LocalFilesystem);
    let Ok(outcome) = host.run(&spec) else {
        return String::new();
    };
    if !outcome.success() {
        return String::new();
    }
    // 空の宣言も1つの宣言である。落として1件に見せると、gitが解決する値と食い違う。
    let stdout = outcome.stdout_text();
    let values: Vec<&str> = stdout.lines().collect();
    let [value] = values.as_slice() else {
        return String::new();
    };
    let value = value.trim();
    if validate_git_identity_value(value).is_err() {
        return String::new();
    }
    value.to_string()
}
