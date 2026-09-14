/// 一覧が示すscopeがglobalか。
///
/// 表の出力は`(global)`と書く。JSONも同じ綴りだが、括弧のない綴りを将来の版が
/// 返しても読めるよう、どちらも受け付ける。Sandbox名がこの綴りになることはない。
pub fn is_global_scope(scope: &str) -> bool {
    matches!(scope, "(global)" | "global")
}
