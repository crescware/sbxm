/// `sbx secret ls`が示すscopeがglobalか。
///
/// 一覧は`(global)`と書く。括弧のない綴りを将来の版が返しても読めるよう、どちらも
/// 受け付ける。Sandbox名がこの綴りになることはない。
pub(super) fn is_global_scope(scope: &str) -> bool {
    matches!(scope, "(global)" | "global")
}
