/// `ssh-add`がagentへ接続できなかったときの終了status。
///
/// 鍵が1件もない場合は`1`で終わるため、接続できたかどうかとは区別できる。
pub const SSH_ADD_NO_AGENT: i32 = 2;
