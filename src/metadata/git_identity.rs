/// `Sandbox内で使用するGit` identity。
///
/// 新規登録時にhostの`git config --global`から取得し、以後は保存値だけを使う。
/// host設定が後から変わっても、登録済み案件のidentityを暗黙変更しない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitIdentity {
    pub user_name: String,
    pub user_email: String,
}
