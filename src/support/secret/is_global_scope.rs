/// `sbx secret ls`が示すscopeがglobalか。
///
/// `sbx secret ls --json`は`(global)`と書く。括弧のない`global`は、その名前の
/// Sandboxのscopeでありうるため、globalとして読まない。
pub(super) fn is_global_scope(scope: &str) -> bool {
    scope == "(global)"
}
